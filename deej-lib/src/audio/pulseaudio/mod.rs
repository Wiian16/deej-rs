use async_trait::async_trait;
use pulseaudio_wrapper::{
    PulseError, PulseWrapper,
    types::{SinkInfo, SourceInfo, Volume},
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

        match target {
            // TODO: currently returns Err on any sink/source failing, should finish the rest of them before returning
            &VolumeTarget::Master => {
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
            &VolumeTarget::Mic => {
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
            _ => {
                log::warn!("Only master channel is implemented for pulseaudio");
                Ok(())
            }
        }
    }
}

impl Into<Volume> for NormalizedVolume {
    fn into(self) -> Volume {
        Volume((self.0 * Volume::NORMAL.0 as f32).round() as u32)
    }
}
