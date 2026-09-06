use std::sync::Arc;

use libpulse_binding::{callbacks::ListResult, context::Context};
use tokio::sync::oneshot;

use crate::audio::pulseaudio::{
    error::PulseError,
    inner::{Command, PulseInner},
    types::SinkInfo,
};

/// Expands to a `move` closure that collects every `ListResult::Item` into a `Vec` (converting
/// it via `From`) and resolves `$tx` with it once the list ends.
macro_rules! list_collector {
    ($tx:expr) => {{
        let mut items = Vec::new();
        let mut tx = Some($tx);
        move |result| match result {
            ListResult::Item(raw) => items.push(::std::convert::From::from(raw)),
            ListResult::End => {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(Ok(::std::mem::take(&mut items)));
                }
            }
            ListResult::Error => {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(Err(PulseError::OperationFailed));
                }
            }
        }
    }};
}

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

    /// Sends a closure to the worker thread and awaits the result it sends back.
    async fn run<F, T>(&self, f: F) -> Result<T, PulseError>
    where
        F: FnOnce(&mut Context, oneshot::Sender<Result<T, PulseError>>) + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.inner
            .send(Command::Run(Box::new(move |ctx| f(ctx, tx))));

        rx.await.map_err(|_| PulseError::Disconnected)?
    }

    pub async fn list_sinks(&self) -> Result<Vec<SinkInfo>, PulseError> {
        self.run(|ctx, tx| {
            ctx.introspect().get_sink_info_list(list_collector!(tx));
        })
        .await
    }
}
