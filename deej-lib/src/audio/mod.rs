use async_trait::async_trait;
use core::fmt;

pub mod pulseaudio;
pub mod volume_registry;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VolumeTarget {
    Master,
    Mic,
    Process(Box<str>),
    Unmapped,
}

impl fmt::Display for VolumeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Master => write!(f, "master"),
            Self::Mic => write!(f, "mic"),
            Self::Process(p) => write!(f, "{p}"),
            Self::Unmapped => write!(f, "deej.unmapped"),
        }
    }
}

#[derive(Debug)]
pub struct AudioAdapterError {
    target: VolumeTarget,
    source: Box<dyn std::error::Error + Send + Sync + 'static>,
}

impl AudioAdapterError {
    pub fn new(
        target: VolumeTarget,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            target,
            source: Box::new(source),
        }
    }
}

impl fmt::Display for AudioAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to set volume for {}: {}",
            self.target, self.source
        )
    }
}

impl std::error::Error for AudioAdapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NormalizedVolume(f32);

impl NormalizedVolume {
    pub const MIN: Self = Self(0.0);
    pub const MAX: Self = Self(1.0);

    #[must_use]
    pub fn new(value: f32) -> Option<Self> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn clamped(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl From<NormalizedVolume> for f32 {
    fn from(volume: NormalizedVolume) -> Self {
        volume.0
    }
}

/// The boundary between the service loop and whatever talks to the platform's audio stack
/// (Pipewire/PulseAudio on Linux, eventually others).
#[async_trait]
pub trait AudioAdapter: Send + Sync {
    async fn set_volume(
        &self,
        target: &VolumeTarget,
        volume: NormalizedVolume,
    ) -> Result<(), AudioAdapterError>;
}

/// No-op logging adapter used while backend is unimplemented.
#[derive(Debug, Default)]
pub struct DummyAudioAdapter;

#[async_trait]
impl AudioAdapter for DummyAudioAdapter {
    async fn set_volume(
        &self,
        target: &VolumeTarget,
        volume: NormalizedVolume,
    ) -> Result<(), AudioAdapterError> {
        log::info!("[dummy audio] {target} -> {:.0}%", volume.get() * 100.0);
        Ok(())
    }
}
