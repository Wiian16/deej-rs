use async_trait::async_trait;
use libpulse_binding::volume::Volume;

use crate::audio::{
    AudioAdapter, AudioAdapterError, NormalizedVolume, VolumeTarget,
    pulseaudio::{error::PulseError, types::SinkInfo, wrapper::PulseWrapper},
    volume_registry::VolumeRegistry,
};

mod error;
mod inner;
mod types;
mod wrapper;

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
