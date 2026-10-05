use async_trait::async_trait;
use pulseaudio_wrapper::{
    PulseError, PulseWrapper, Subscription,
    types::{
        ChannelVolumes, Facility, InterestMask, InterestMaskSetBuilder, Operation, SinkInfo,
        SinkInputInfo, SourceInfo, SubscriptionEvent, Volume,
    },
};
use tokio_util::sync::CancellationToken;

use crate::audio::{
    AudioAdapter, AudioAdapterError, NormalizedVolume, ProcessName, VolumeTarget,
    volume_registry::VolumeRegistry,
};

#[allow(clippy::as_conversions)]
const PA_VOLUME_NORM: u64 = Volume::NORMAL.0 as u64;

// Property keys to filter processes by.
const APP_NAME: &str = "application.name";
const APP_BINARY: &str = "application.process.binary";
const NODE_NAME: &str = "node.name";

#[derive(Clone)]
pub struct PulseAudioAdapter {
    wrapper: PulseWrapper,
    registry: VolumeRegistry,
}

impl PulseAudioAdapter {
    /// Creates a new `PulseAudioAdapter`.
    ///
    /// # Errors
    ///
    /// Returns [`PulseError`] if there is an error in the creation of the `PulseAudio` service.
    pub async fn new(shutdown: CancellationToken) -> Result<Self, PulseError> {
        let wrapper = PulseWrapper::new("deej-pulseaudio-adapter".into()).await?;

        let interests = InterestMaskSetBuilder::new()
            .set(InterestMask::Sink)
            .set(InterestMask::Source)
            .set(InterestMask::SinkInput)
            .build();
        let sub = wrapper.subscribe(interests).await?;
        let adapter = Self {
            wrapper,
            registry: VolumeRegistry::new(),
        };

        tokio::spawn(subscription_task(adapter.clone(), sub, shutdown.clone()));

        Ok(adapter)
    }

    pub async fn handle_subscription_event(&self, event: SubscriptionEvent) {
        if event.operation == Operation::Removed {
            return;
        }

        // FIXME: add check for if volume needs to be changed (is outside a small window, maybe 2%?)

        let result = match event.facility {
            Facility::Sink => self.apply_sink_volume(event.index).await,
            Facility::Source => self.apply_source_volume(event.index).await,
            Facility::SinkInput => self.apply_sink_input_volume(event.index).await,
            _ => Ok(()),
        };

        if let Err(err) = result {
            log::warn!(
                "failed to update {:?} (index {}): {err:#}",
                event.facility, // FIXME: would rather use display here, not debug, but display isn't implemented
                event.index
            );
        }
    }

    async fn apply_sink_volume(&self, index: u32) -> Result<(), PulseError> {
        let Some(volume) = self.registry.resolve(&VolumeTarget::Master) else {
            return Ok(());
        };

        let sink = self.wrapper.sink_by_index(index).await?;

        if volume_is_already_correct(&sink.volume, volume.into(), Some(sink.n_volume_steps)) {
            return Ok(());
        }

        let mut volumes = sink.volume;
        volumes.set(volumes.len(), volume.into());
        self.wrapper.set_sink_volume(index, volumes).await
    }

    async fn apply_source_volume(&self, index: u32) -> Result<(), PulseError> {
        let Some(volume) = self.registry.resolve(&VolumeTarget::Mic) else {
            return Ok(());
        };

        let source = self.wrapper.source_by_index(index).await?;

        if volume_is_already_correct(&source.volume, volume.into(), Some(source.n_volume_steps)) {
            return Ok(());
        }

        let mut volumes = source.volume;
        volumes.set(volumes.len(), volume.into());
        self.wrapper.set_source_volume(index, volumes).await
    }

    async fn apply_sink_input_volume(&self, index: u32) -> Result<(), PulseError> {
        let stream = self.wrapper.sink_input_by_index(index).await?;

        let Some(volume) = self.resolve_stream_volume(&stream) else {
            return Ok(());
        };

        if volume_is_already_correct(&stream.volume, volume.into(), None) {
            return Ok(());
        }

        let mut volumes = stream.volume;
        volumes.set(volumes.len(), volume.into());
        self.wrapper.set_sink_input_volume(index, volumes).await
    }

    /// Determines if a sink input is unmapped based off the volume registry.
    ///
    /// Uses `application.name`, `application.process.binary`, and `node.name` from the proplist to check against the
    /// volume registry to determine if it is unmapped. If none of these fields exactly resolve to a process, it is
    /// considered to be unmapped.
    fn is_unmapped(&self, sink_input_info: &SinkInputInfo) -> bool {
        let proplist = &sink_input_info.properties;

        let app_name_match = proplist.get(APP_NAME).is_some_and(|app_name| {
            self.registry
                .resolve_process_exact(&ProcessName::new(app_name.as_ref()))
                .is_some()
        });
        let app_binary_match = proplist.get(APP_BINARY).is_some_and(|app_binary| {
            self.registry
                .resolve_process_exact(&ProcessName::new(app_binary.as_ref()))
                .is_some()
        });
        let node_name_match = proplist.get(NODE_NAME).is_some_and(|node_name| {
            self.registry
                .resolve_process_exact(&ProcessName::new(node_name.as_ref()))
                .is_some()
        });

        !(app_name_match || app_binary_match || node_name_match)
    }

    /// Resolves the volume that should apply to `stream`.
    /// Checks `application.name`, `application.process.binary`, and `node.name` against registered process volumes in
    /// turn, falling back to the unmapped volume if none of them match.
    fn resolve_stream_volume(&self, stream: &SinkInputInfo) -> Option<NormalizedVolume> {
        let proplist = &stream.properties;

        [APP_NAME, APP_BINARY, NODE_NAME]
            .into_iter()
            .find_map(|key| {
                let value = proplist.get(key)?;
                self.registry
                    .resolve_process_exact(&ProcessName::new(value.as_ref()))
            })
            .or_else(|| self.registry.resolve_exact(&VolumeTarget::Unmapped))
    }
}

#[async_trait]
impl AudioAdapter for PulseAudioAdapter {
    async fn set_volume(
        &self,
        target: &VolumeTarget,
        volume: NormalizedVolume,
    ) -> Result<(), AudioAdapterError> {
        self.registry.register(target.clone(), volume);

        match *target {
            // TODO: currently returns Err on any sink/source failing, should finish the rest of them before returning
            VolumeTarget::Master => {
                let sinks: Vec<SinkInfo> = self
                    .wrapper
                    .list_sinks()
                    .await
                    .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                let target_volume: Volume = volume.into();

                for sink in sinks {
                    let index = sink.index;
                    let mut volumes = sink.volume;
                    volumes.set(volumes.len(), target_volume);

                    self.wrapper
                        .set_sink_volume(index, volumes)
                        .await
                        .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                }
            }
            VolumeTarget::Mic => {
                let sources: Vec<SourceInfo> = self
                    .wrapper
                    .list_sources()
                    .await
                    .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                let target_volume: Volume = volume.into();

                for source in sources {
                    let index = source.index;
                    let mut volumes = source.volume;
                    volumes.set(volumes.len(), target_volume);

                    self.wrapper
                        .set_source_volume(index, volumes)
                        .await
                        .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                }
            }
            VolumeTarget::Process(ref process_name) => {
                let streams = self
                    .wrapper
                    .list_sink_inputs()
                    .await
                    .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                let target_volume: Volume = volume.into();

                let filtered: Vec<&SinkInputInfo> = streams
                    .iter()
                    .filter(|stream| match_process(process_name.name(), stream))
                    .collect();

                for stream in filtered {
                    let index = stream.index;
                    let mut volumes = stream.volume;
                    volumes.set(volumes.len(), target_volume);

                    self.wrapper
                        .set_sink_input_volume(index, volumes)
                        .await
                        .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                }
            }
            VolumeTarget::Unmapped => {
                let streams = self
                    .wrapper
                    .list_sink_inputs()
                    .await
                    .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                let target_volume: Volume = volume.into();

                let filtered = streams.iter().filter(|stream| self.is_unmapped(stream));

                for stream in filtered {
                    let index = stream.index;
                    let mut volumes = stream.volume;
                    volumes.set(volumes.len(), target_volume);

                    self.wrapper
                        .set_sink_input_volume(index, volumes)
                        .await
                        .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                }
            }
        }

        Ok(())
    }
}

/// Determines if a sink input belongs to a process with `name`.
///
/// Checks `application.name`, `application.process.binary`, and `node.name` from the proplist to match with `name`. If
/// any one of those properties matches, the sink input is considered matched.
fn match_process(name: &str, sink_input_info: &SinkInputInfo) -> bool {
    let proplist = &sink_input_info.properties;

    let app_name_match = proplist
        .get(APP_NAME)
        .is_some_and(|value| value.eq_ignore_ascii_case(name));
    let app_binary_match = proplist
        .get(APP_BINARY)
        .is_some_and(|value| value.eq_ignore_ascii_case(name));
    let node_name_match = proplist
        .get(NODE_NAME)
        .is_some_and(|value| value.eq_ignore_ascii_case(name));

    app_name_match || app_binary_match || node_name_match
}

/// Converts `volume` to a new quantized volume when given `steps`, typically from [`SinkInfo`] or [`SourceInfo`]
/// `n_volume_steps` field.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation
)]
fn hardware_step(volume: Volume, steps: u32) -> Volume {
    let intervals = u64::from(steps - 1);
    let step = (u64::from(volume.0) * intervals + PA_VOLUME_NORM / 2) / PA_VOLUME_NORM;
    Volume(step.min(intervals) as u32)
}

/// Determines if an object's volume is already at the target volume considering it's discrete volume steps.
///
/// For objects that don't have discrete volume steps, such as `SinkInput`, pass `None` for `n_volume_steps`.
///
/// [`SinkInfo`] and [`SourceInfo`]'s `n_volume_steps` indicate how many discrete steps a device may have if it doesn't
/// support arbitrary volume. This leaves 3 cases:
///
/// 1. `n_volume_steps == 1`: only one state, all volumes are equivalent
///
/// 2. `2 <= n_volume_steps <= 65536`: some discrete number of steps, convert target volume to quantized volume based on
///    steps and compare
///
/// 3. `n_volume_steps == 65537`: supports arbitrary volume, compare volumes as-is
fn volume_is_already_correct(
    current: &ChannelVolumes,
    target: Volume,
    n_volume_steps: Option<u32>,
) -> bool {
    match n_volume_steps {
        Some(1) => true,
        Some(n @ 2..=65_536) => {
            let target_step = hardware_step(target, n);

            current
                .get()
                .iter()
                .all(|current| hardware_step(*current, n) == target_step)
        }
        _ => current.get().iter().all(|current| *current == target),
    }
}

async fn subscription_task(
    adapter: PulseAudioAdapter,
    mut sub: Subscription,
    shutdown: CancellationToken,
) {
    loop {
        let event = tokio::select! {
            () = shutdown.cancelled() => break,
            event = sub.recv() => event,
        };

        match event {
            Some(event) => {
                // spawn a new task to continue processing events instead of waiting for this one to complete.
                let handler_adapter = adapter.clone();
                tokio::spawn(async move {
                    handler_adapter.handle_subscription_event(event).await;
                });
            }
            // sender was dropped, shouldn't happen, but don't spin forever if it does.
            None => break,
        }
    }

    log::debug!("pulseaudio subscription task shut down");
}

impl From<NormalizedVolume> for Volume {
    fn from(val: NormalizedVolume) -> Self {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            clippy::as_conversions
        )]
        Self((val.0 * Self::NORMAL.0 as f32).round() as u32)
    }
}

#[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
#[cfg(test)]
const NORM: u32 = PA_VOLUME_NORM as u32;

#[cfg(test)]
mod test_hardware_step {
    use super::*;

    #[test]
    fn single_step() {
        for volume in [0, 1, NORM / 2, NORM, u32::MAX] {
            assert_eq!(
                hardware_step(Volume(volume), 1),
                Volume(0),
                "devices with a single step should always map to 0 volume"
            );
        }
    }

    #[test]
    fn two_steps() {
        // Should round about the midpoint
        assert_eq!(
            hardware_step(Volume(0), 2),
            Volume(0),
            "volumes under the midpoint should map to 0 volume at 2 hardware steps"
        );
        assert_eq!(
            hardware_step(Volume(NORM / 2 - 1), 2),
            Volume(0),
            "volumes under the midpoint should round down to 0 at 2 hardware steps"
        );
        assert_eq!(
            hardware_step(Volume(NORM / 2), 2),
            Volume(1),
            "volumes at the midpoint should round up to 1 at 2 hardware steps"
        );
        assert_eq!(
            hardware_step(Volume(NORM), 2),
            Volume(1),
            "volumes above the midpoint should map to 1 at 2 hardware steps"
        );
    }

    #[test]
    fn three_steps() {
        // intervals = 2, so midpoints are at NORM/4 and 3*NORM/4
        assert_eq!(
            hardware_step(Volume(NORM / 4 - 1), 3),
            Volume(0),
            "volumes under the first midpoint should round to 0 at 3 hardware steps"
        );
        assert_eq!(
            hardware_step(Volume(NORM / 4), 3),
            Volume(1),
            "volumes at the first midpoint should map to 1 at 3 hardware steps"
        );
        assert_eq!(
            hardware_step(Volume(3 * NORM / 4 - 1), 3),
            Volume(1),
            "volumes below the second midpoint should map to 1 at 3 hardware steps"
        );
        assert_eq!(
            hardware_step(Volume(3 * NORM / 4), 3),
            Volume(2),
            "volumes at the second midpoint should map to 2 at 3 hardware steps"
        );
    }

    #[test]
    fn overamplified_clamps() {
        assert_eq!(
            hardware_step(Volume(NORM + 1), 11),
            Volume(10),
            "volumes over normal should clamp to the max hardware step"
        );
        assert_eq!(
            hardware_step(Volume(NORM * 2), 11),
            Volume(10),
            "volumes over normal should clamp to the max hardware step"
        );
        assert_eq!(
            hardware_step(Volume(u32::MAX), 11),
            Volume(10),
            "volumes over normal should clamp to the max hardware step"
        );
    }

    #[test]
    fn max_steps() {
        assert_eq!(
            hardware_step(Volume(0), 65_536),
            Volume(0),
            "volumes at max hardware steps should stay in the normal range"
        );
        assert_eq!(
            hardware_step(Volume(NORM), 65_536),
            Volume(65_535),
            "volumes at max hardware steps should stay in the normal range"
        );
    }
}

#[cfg(test)]
mod test_volume_is_already_correct {
    use super::*;

    #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
    /// Build a [`ChannelVolumes`] with one entry per value in `volumes`.
    fn channels(volumes: &[u32]) -> ChannelVolumes {
        let mut cv = ChannelVolumes::default();
        cv.set(volumes.len() as u8, Volume(0));

        for (slot, volume) in cv.get_mut().iter_mut().zip(volumes) {
            *slot = Volume(*volume);
        }

        cv
    }

    #[test]
    fn none_compares_exactly() {
        let current = channels(&[1000, 1000]);

        assert!(
            volume_is_already_correct(&current, Volume(1000), None),
            "devices with arbitrary volume support should compare exactly"
        );
        assert!(
            !volume_is_already_correct(&current, Volume(1001), None),
            "devices with arbitrary volume support should compare exactly"
        );
        assert!(
            !volume_is_already_correct(&current, Volume(999), None),
            "devices with arbitrary volume support should compare exactly"
        );
    }

    #[test]
    fn none_fails_if_any_channel_fails() {
        let current = channels(&[1000, 1000, 999]);

        assert!(
            !volume_is_already_correct(&current, Volume(1000), None),
            "any channel that is not equal should fail"
        );
    }

    #[test]
    fn one_step_always_correct() {
        let current = channels(&[0, NORM, u32::MAX]);

        assert!(
            volume_is_already_correct(&current, Volume(12345), Some(1)),
            "devices only supporting one volume step should always return true"
        );
    }

    #[test]
    fn discrete_steps_treat_same_bucket_as_equal() {
        // 3 steps: 0 and NORM/4 - 1 both land on step 0
        let cur = channels(&[0, NORM / 4 - 1]);
        assert!(
            volume_is_already_correct(&cur, Volume(100), Some(3)),
            "volumes that land in the same interval with discrete steps should be equal"
        );
    }

    #[test]
    fn discrete_steps_detect_different_bucket() {
        let cur = channels(&[NORM / 4, NORM / 4]); // step 1
        assert!(
            !volume_is_already_correct(&cur, Volume(0), Some(3)),
            "volumes that land in different intervals with discrete steps should not be equal"
        );
    }

    #[test]
    fn discrete_steps_one_channel_in_wrong_bucket_fails() {
        let cur = channels(&[0, 0, NORM]);
        assert!(
            !volume_is_already_correct(&cur, Volume(0), Some(3)),
            "any channel that is not equal should fail"
        );
    }

    #[test]
    fn match_arm_boundaries() {
        let cur = channels(&[NORM / 2]);
        let target = Volume(NORM / 2 + 1);

        // 2 and 65_536 are quantized: the values are near-equal but not identical
        assert!(volume_is_already_correct(&cur, target, Some(2)));
        #[allow(clippy::manual_assert_eq)]
        {
            assert!(
                volume_is_already_correct(&cur, target, Some(65_536))
                    == (hardware_step(Volume(NORM / 2), 65_536) == hardware_step(target, 65_536))
            );
        }

        // 65_537 (arbitrary volume) and out-of-range values compare exactly
        assert!(!volume_is_already_correct(&cur, target, Some(65_537)));
        assert!(!volume_is_already_correct(&cur, target, Some(0)));
        assert!(!volume_is_already_correct(&cur, target, Some(100_000)));
        assert!(volume_is_already_correct(
            &cur,
            Volume(NORM / 2),
            Some(65_537)
        ));
    }
}
