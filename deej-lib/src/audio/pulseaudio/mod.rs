use std::collections::HashMap;

use async_trait::async_trait;
use pulseaudio_wrapper::{
    PulseError, PulseWrapper,
    types::{SinkInfo, SinkInputInfo, SourceInfo, Volume},
};

use crate::audio::{
    AudioAdapter, AudioAdapterError, NormalizedVolume, VolumeTarget,
    volume_registry::VolumeRegistry,
};

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
    pub async fn new() -> Result<Self, PulseError> {
        Ok(Self {
            wrapper: PulseWrapper::new("deej-pulseaudio-adapter".into()).await?,
            registry: VolumeRegistry::new(),
        })
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

                Ok(())
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

                Ok(())
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
                    .filter(|stream| match_process(name, &stream.properties))
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

                Ok(())
            }
            VolumeTarget::Unmapped => Err(AudioAdapterError::without_source(
                target.clone(),
                "not implemented",
            )),
        }
    }
}

fn match_process(name: &str, proplist: &HashMap<Box<str>, Box<str>>) -> bool {
    // Property keys to filter by.
    const APP_NAME: &str = "application.name";
    const APP_BINARY: &str = "application.process.binary";
    const NODE_NAME: &str = "node.name";

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
