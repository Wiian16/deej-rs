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
    message: Option<Box<str>>,
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl AudioAdapterError {
    pub fn new(
        target: VolumeTarget,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            target,
            message: None,
            source: Some(Box::new(source)),
        }
    }

    pub fn without_source(target: VolumeTarget, message: impl Into<Box<str>>) -> Self {
        Self {
            target,
            message: Some(message.into()),
            source: None,
        }
    }

    #[must_use]
    pub fn with_message(mut self, message: impl Into<Box<str>>) -> Self {
        self.message = Some(message.into());
        self
    }

    #[must_use]
    pub const fn target(&self) -> &VolumeTarget {
        &self.target
    }
}

impl fmt::Display for AudioAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to set volume for {}", self.target)?;
        if let Some(message) = &self.message {
            write!(f, ": {message}")?;
        }
        if let Some(source) = &self.source {
            write!(f, ": {source}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AudioAdapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        #[allow(clippy::option_map_or_none)]
        self.source
            .as_ref()
            .map_or(None, |source| Some(source.as_ref()))
    }
}

/// Represents a volume as a percentage.
///
/// `NormalizedVolumes` will always be between 0.0 (0%) and 1.0 (100%) inclusive. Use [`NormalizedVolume::new()`] to
/// create a normalized volume, failing if the value is not normalized. Use [`NormalizedVolume::clamped()`] to create a
/// normalized volume that is clamped to be in [0.0, 1.0].
///
/// # Examples
/// ```
/// use deej_lib::audio::NormalizedVolume;
///
/// // Valid values construct successfully.
/// let half = NormalizedVolume::new(0.5).unwrap();
/// assert!(half.get() - 0.5 < 0.001);
///
/// // Out-of-range values are rejected.
/// assert!(NormalizedVolume::new(1.5).is_none());
/// assert!(NormalizedVolume::new(-0.1).is_none());
///
/// // Non-finite values are also rejected.
/// assert!(NormalizedVolume::new(f32::NAN).is_none());
/// assert!(NormalizedVolume::new(f32::INFINITY).is_none());
///
/// // `clamped` never fails, instead saturating to the valid range.
/// assert_eq!(NormalizedVolume::clamped(1.5), NormalizedVolume::MAX);
/// assert_eq!(NormalizedVolume::clamped(-0.1), NormalizedVolume::MIN);
/// assert!(NormalizedVolume::clamped(0.5).get() - 0.5 < 0.001);
///
/// // Converts back into a plain `f32` when needed.
/// let volume: f32 = half.into();
/// assert!(volume - 0.5 < 0.001);
/// ```
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

    /// Create a new `NormalizedVolume` that is always clamped between 0.0 and 1.0 inclusive.
    ///
    /// Unlike [`NormalizedVolume::new()`], this constructor always succeeds. [`f32::NAN`] is clamped to 0.0, rather than propagating like in [`f32::clamp`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deej_lib::audio::NormalizedVolume;
    ///
    /// // Values in range are unaffected
    /// assert!(NormalizedVolume::clamped(0.5).get() - 0.5 < 0.001);
    ///
    /// // Out-of-range values clamp to the nearest bound
    /// assert_eq!(NormalizedVolume::clamped(1.5), NormalizedVolume::MAX);
    /// assert_eq!(NormalizedVolume::clamped(-0.1), NormalizedVolume::MIN);
    ///
    /// // NaN clamps to MIN, rather than propagating
    /// assert_eq!(NormalizedVolume::clamped(f32::NAN), NormalizedVolume::MIN);
    /// ```
    #[must_use]
    pub const fn clamped(value: f32) -> Self {
        if value.is_nan() {
            Self::MIN
        } else {
            Self(value.clamp(0.0, 1.0))
        }
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
