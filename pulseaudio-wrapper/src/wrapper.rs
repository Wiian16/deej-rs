use std::sync::Arc;

use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
};
use libpulse_binding::{callbacks::ListResult, context::Context, volume::ChannelVolumes};

use crate::{
    dispatcher::{SubDispatcher, SubscriptionId},
    error::PulseError,
    inner::{Command, PulseInner},
    types::{InterestMaskSet, SinkInfo, SinkInputInfo, SourceInfo, SubscriptionEvent},
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

/// An async handle to a `PulseAudio` server connection.
///
/// `PulseWrapper` is cheap to [`Clone`] (it's just an `Arc` around a channel to the worker thread), so it's fine to
/// share across tasks. The connection is closed automatically and the thread is joined when the last clone is dropped.
#[derive(Clone)]
pub struct PulseWrapper {
    inner: Arc<PulseInner>,
    dispatcher: SubDispatcher,
}

impl PulseWrapper {
    /// Creates a new handle into a new worker thread.
    ///
    /// # Errors
    ///
    /// Returns [`PulseError`] if the worker thread fails to spawn. See type for more information.
    pub async fn new(name: String) -> Result<Self, PulseError> {
        let inner = PulseInner::new(name).await?;
        let dispatcher = SubDispatcher::new();

        // Register the subscription callback to dispatcher.dispatch()
        let callback_dispatcher = dispatcher.clone();
        inner.send(Command::Run(Box::new(move |ctx| {
            ctx.set_subscribe_callback(Some(Box::new(move |facility, operation, index| {
                if let (Some(facility), Some(operation)) = (facility, operation) {
                    callback_dispatcher.dispatch(SubscriptionEvent {
                        facility,
                        operation,
                        index,
                    });
                }
            })));
        })));

        Ok(Self { inner, dispatcher })
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

    /// Get the list of sinks connected to the `PulseAudio` server.
    ///
    /// # Errors
    ///
    /// Returns [`PulseError`] if the operation fails or the connection fails during the operation. See type for more
    /// information.
    pub async fn list_sinks(&self) -> Result<Vec<SinkInfo>, PulseError> {
        self.run(|ctx, tx| {
            ctx.introspect().get_sink_info_list(list_collector!(tx));
        })
        .await
    }

    /// Get a sink connected to the `PulseAudio` server by index.
    ///
    /// # Errors
    ///
    /// If the sink cannot be found, returns [`PulseError::NotFound`].
    ///
    /// Returns [`PulseError`] if the operation fails or the connection fails during the operation. See type for more
    /// information.
    pub async fn sink_by_index(&self, index: u32) -> Result<SinkInfo, PulseError> {
        self.run(move |ctx, tx| {
            ctx.introspect()
                .get_sink_info_by_index(index, single_collector!(tx));
        })
        .await
    }

    /// Set a sink's volume by index.
    ///
    /// See [`ChannelVolumes`] for information on volume information.
    ///
    /// # Errors
    ///
    /// If the sink cannot be found, may return [`PulseError::OperationFailed`].
    ///
    /// Returns [`PulseError`] if the operation fails or the connection fails during the operation. See type for more
    /// information.
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

    /// Get the list of sources connected to the `PulseAudio` server.
    ///
    /// # Errors
    ///
    /// Returns [`PulseError`] if the operation fails or the connection fails during the operation. See type for more
    /// information.
    pub async fn list_sources(&self) -> Result<Vec<SourceInfo>, PulseError> {
        self.run(|ctx, tx| {
            ctx.introspect().get_source_info_list(list_collector!(tx));
        })
        .await
    }

    /// Set a source's volume by index.
    ///
    /// See [`ChannelVolumes`] for information on volume information.
    ///
    /// # Errors
    ///
    /// If the sink cannot be found, may return [`PulseError::OperationFailed`].
    ///
    /// Returns [`PulseError`] if the operation fails or the connection fails during the operation. See type for more
    /// information.
    pub async fn set_source_volume(
        &self,
        index: u32,
        volume: ChannelVolumes,
    ) -> Result<(), PulseError> {
        self.run(move |ctx, tx| {
            let _ = ctx.introspect().set_source_volume_by_index(
                index,
                &volume,
                Some(success_callback(tx)),
            );
        })
        .await
    }

    /// Get the list of all sink inputs connected to the `PulseAudio` server.
    ///
    /// # Errors
    ///
    /// Returns [`PulseError`] if the operation fails or the connection fails during the operation. See type for more
    /// information.
    pub async fn list_sink_inputs(&self) -> Result<Vec<SinkInputInfo>, PulseError> {
        self.run(move |ctx, tx| {
            let _ = ctx
                .introspect()
                .get_sink_input_info_list(list_collector!(tx));
        })
        .await
    }

    /// Set a sink input's volume by index.
    ///
    /// See [`ChannelVolumes`] for information on volume information.
    ///
    /// # Errors
    ///
    /// If the source input cannot be found, may return [`PulseError::OperationFailed`].
    ///
    /// Returns [`PulseError`] if the operation fails or the connection fails during the operation. See type for more
    /// information.
    pub async fn set_sink_input_volume(
        &self,
        index: u32,
        volume: ChannelVolumes,
    ) -> Result<(), PulseError> {
        self.run(move |ctx, tx| {
            let _ =
                ctx.introspect()
                    .set_sink_input_volume(index, &volume, Some(success_callback(tx)));
        })
        .await
    }

    /// Creates a new subscription to events from [facilities](pulseaudio-wrapper::types::Facility) specified in `mask`.
    ///
    /// Returns a [`Subscription`] handle. Call [`Subscription::recv`] to await matching events. Dropping the handle
    /// automatically unsubscribes, and (if no other subscriber still wants them) tells the server to stop sending
    /// events for the facilities it held.
    ///
    /// # Errors
    ///
    /// Returns [`PulseError`] if updating the server's subscription mask fails, or the connection fails during the
    /// operation. See type for more information.
    pub async fn subscribe(&self, mask: InterestMaskSet) -> Result<Subscription, PulseError> {
        let result = self.dispatcher.register(mask);

        if result.grew
            && let Err(err) = self.update_subscription_mask().await
        {
            // Don't leave a registered mask when the update failed to apply.
            self.dispatcher.unregister(result.id);
            return Err(err);
        }

        Ok(Subscription {
            id: result.id,
            wrapper: self.clone(),
            rx: result.rx,
        })
    }

    /// Called when [`Subscription`]s are dropped.
    ///
    /// Unregisters the corresponding mask from the dispatcher and drops the corresponding sender. Because this method
    /// is called from [`Drop::drop`], it should be async or fail. We use [`PulseInner::send`] for this and don't
    /// listen to the callback to accomplish this. See note in function body for more info.  
    fn unsubscribe(&self, id: SubscriptionId) {
        let shrank = self.dispatcher.unregister(id);

        if shrank {
            let mask = self.dispatcher.current_mask();

            // self.inner.send is a non-async function, all we want to do when dropping is fire-and-forget updating the
            // mask. If it fails to update, no events will be missed, we only risk filtering out extraneous events.
            // If there is an issue with communicating with the server, it can be handled next time a crucial function
            // needs to be run.
            self.inner.send(Command::Run(Box::new(move |ctx| {
                ctx.subscribe(mask.into(), |_| {});
            })));
        }
    }

    /// Pushes the dispatcher's current interest mask to the server.
    ///
    /// # Errors
    ///
    /// Returns [`PulseError`] if updating the mask fails, or the connection fails during the operation. See type for
    /// more information.
    pub async fn update_subscription_mask(&self) -> Result<(), PulseError> {
        let mask = self.dispatcher.current_mask();
        self.run(move |ctx, tx| {
            ctx.subscribe(mask.into(), success_callback(tx));
        })
        .await
    }
}
pub struct Subscription {
    id: SubscriptionId,
    wrapper: PulseWrapper,
    rx: mpsc::Receiver<SubscriptionEvent>,
}

/// A subscription to `PulseAudio` events matching the [`InterestMaskSet`] it was created with.
///
/// Call [`recv`](Self::recv) to await the next matching event.
///
/// Automatically unsubscribes when dropped. The local registration is removed synchronously, and if that was the last
/// subscriber interested in one of it's facilities, an update is enqueued on the worker thread to tell the server to
/// stop sending those events.
impl Subscription {
    /// Awaits the next event matching this subscription's interest mask.
    ///
    /// Returns `None` once the dispatcher's sender half is gone (subscription has been torn down). This shouldn't
    /// happen in practice while the `Subscription` is still alive, since only it's own `Drop` removes it.
    pub async fn recv(&mut self) -> Option<SubscriptionEvent> {
        self.rx.next().await
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.wrapper.unsubscribe(self.id);
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
