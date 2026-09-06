use std::sync::Arc;

use crate::audio::pulseaudio::{error::PulseError, inner::PulseInner};

/// An async handle to a PulseAudio server connection.
///
/// `PulseWrapper` is cheap to [`Clone`] (it's just an `Arc` around a channel to the worker thread), so it's fine to
/// share across tasks. The connection is closed automatically and the thread is joined when the last clone is dropped.
#[derive(Clone)]
pub(crate) struct PulseWrapper {
    inner: Arc<PulseInner>,
}

impl PulseWrapper {
    pub async fn new(name: String) -> Result<Self, PulseError> {
        Ok(Self {
            inner: PulseInner::new(name).await?,
        })
    }
}
