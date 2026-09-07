use std::sync::Arc;

use libpulse_binding::{callbacks::ListResult, context::Context, volume::ChannelVolumes};
use tokio::sync::oneshot;

use crate::audio::pulseaudio::{
    error::PulseError,
    inner::{Command, PulseInner},
    types::SinkInfo,
};

/// Expands to a `move` closure that collects every `ListResult::Item` into a `Vec` (converting it via `From`) and
/// resolves `$tx` with it once the list ends.
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

/// Expands to a `move` closure that keeps the last `ListResult::Item` it saw (converting it via `From`) and resolves
/// `$tx` with it once the list ends (or [`PulseError::NotFound`] if no item ever arrived).
macro_rules! single_collector {
    ($tx:expr) => {{
        let mut found = None;
        let mut tx = Some($tx);
        move |result| match result {
            ListResult::Item(raw) => found = Some(::std::convert::From::from(raw)),
            ListResult::End => {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(found.take().ok_or(PulseError::NotFound));
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

    pub async fn sink_by_index(&self, index: u32) -> Result<SinkInfo, PulseError> {
        self.run(move |ctx, tx| {
            ctx.introspect()
                .get_sink_info_by_index(index, single_collector!(tx));
        })
        .await
    }

    pub async fn set_sink_volume(
        &self,
        index: u32,
        volume: ChannelVolumes,
    ) -> Result<(), PulseError> {
        self.run(move |ctx, tx| {
            let _ = ctx.introspect().set_sink_volume_by_index(
                index,
                &volume,
                Some(success_callback(tx)),
            );
        })
        .await
    }
}

/// Wrap a one-shot sender into the `FnMut(bool)` shape every set, move, and kill introspection calls want for their
/// success callbacks.
fn success_callback(tx: oneshot::Sender<Result<(), PulseError>>) -> Box<dyn FnMut(bool) + 'static> {
    let mut tx = Some(tx);

    Box::new(move |success: bool| {
        if let Some(tx) = tx.take() {
            let _ = tx.send(if success {
                Ok(())
            } else {
                Err(PulseError::OperationFailed)
            });
        }
    })
}
