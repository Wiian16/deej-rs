use async_trait::async_trait;
use core::fmt;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VolumeTarget {
    Master,
    Mic,
    Process(String),
    Group(Vec<String>),
    Unmapped,
}

impl fmt::Display for VolumeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Master => write!(f, "master"),
            Self::Mic => write!(f, "mic"),
            Self::Process(p) => write!(f, "{p}"),
            Self::Group(procs) => write!(f, "{}", procs.join("+")),
            Self::Unmapped => write!(f, "deej.unmapped"),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NoiseReduction {
    Low,
    Default,
    High,
}

pub enum AudioAdapterError {}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NormalizedVolume(f32);

impl NormalizedVolume {
    pub const MIN: Self = Self(0.0);
    pub const MAX: Self = Self(1.0);

    pub fn new(value: f32) -> Option<Self> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    pub fn clamped(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

impl From<NormalizedVolume> for f32 {
    fn from(volume: NormalizedVolume) -> f32 {
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
