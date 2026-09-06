use async_trait::async_trait;

use crate::audio::{
    AudioAdapter, AudioAdapterError, NormalizedVolume, VolumeTarget,
    pulseaudio::{error::PulseError, wrapper::PulseWrapper},
};

mod error;
mod inner;
mod wrapper;

pub struct PulseAudioAdapter {
    wrapper: PulseWrapper,
}

impl PulseAudioAdapter {
    pub async fn new() -> Result<Self, PulseError> {
        Ok(Self {
            wrapper: PulseWrapper::new("deej-pulseaudio-adapter".into()).await?,
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
        todo!()
    }
}
