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
    AudioAdapter, AudioAdapterError, NormalizedVolume, VolumeTarget,
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
                .resolve_process_exact(&app_name.to_lowercase())
                .is_some()
        });
        let app_binary_match = proplist.get(APP_BINARY).is_some_and(|app_binary| {
            self.registry
                .resolve_process_exact(&app_binary.to_lowercase())
                .is_some()
        });
        let node_name_match = proplist.get(NODE_NAME).is_some_and(|node_name| {
            self.registry
                .resolve_process_exact(&node_name.to_lowercase())
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
                self.registry.resolve_process_exact(&value.to_lowercase())
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
            VolumeTarget::Process(ref name) => {
                let streams = self
                    .wrapper
                    .list_sink_inputs()
                    .await
                    .map_err(|err| AudioAdapterError::new(target.clone(), err))?;
                let target_volume: Volume = volume.into();

                let filtered: Vec<&SinkInputInfo> = streams
                    .iter()
                    .filter(|stream| match_process(name, stream))
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
