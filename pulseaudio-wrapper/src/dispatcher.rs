use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::channel::mpsc;

use crate::types::{InterestMask, InterestMaskSet, InterestMaskSetBuilder, SubscriptionEvent};

const CHANNEL_BOUND: usize = 32;

#[derive(Default)]
struct State {
    interest_count: HashMap<InterestMask, usize>,
    subscribers: Vec<(
        SubscriptionId,
        InterestMaskSet,
        mpsc::Sender<SubscriptionEvent>,
    )>,
}

/// The result of [`SubDispatcher::register`].
///
/// Contains the id of the newly registered subscription (`id`), the receiver for the subscription (`rx`), and whether
/// the union of the `InterestMask`s grew (`grew`).
pub struct RegisterResult {
    pub id: SubscriptionId,
    pub rx: mpsc::Receiver<SubscriptionEvent>,
    pub grew: bool,
}

pub type SubscriptionId = u64;

/// Dispatches [`SubscriptionEvent`]s to subscribers based on the [`InterestMask`] they subscribed with and tracks the
/// union of every subscription's `InterestMask`.
#[derive(Clone)]
pub struct SubDispatcher {
    next_id: Arc<AtomicU64>,
    state: Arc<Mutex<State>>,
}

impl SubDispatcher {
    pub fn new() -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(0)),
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    pub fn dispatch(&self, event: SubscriptionEvent) {
        let mut state = self.state.lock().expect("poisoned mutex");

        state
            .subscribers
            .iter_mut()
            .filter(|(_id, mask, _tx)| mask.contains(event.facility.into()))
            .for_each(|(_id, _mask, tx)| {
                if let Err(err) = tx.try_send(event) {
                    if err.is_full() {
                        log::warn!("subscription buffer is full: dropping event");
                    } else {
                        log::warn!("error sending event to subscription: {err}");
                    }
                }
            });
    }

    /// Registers interest in `mask`.
    ///
    /// Returns a fresh id, the reciever to be used with that id, and whether the union of all interest masks grew by
    /// adding this interest.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn register(&self, mask: InterestMaskSet) -> RegisterResult {
        let (tx, rx) = mpsc::channel::<SubscriptionEvent>(CHANNEL_BOUND);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let mut state = self.state.lock().expect("poisoned mutex");
        let mut grew = false;

        for interest in mask {
            let count = state.interest_count.entry(interest).or_insert(0);

            if *count == 0 {
                grew = true;
            }

            *count += 1;
        }

        state.subscribers.push((id, mask, tx));
        drop(state);

        RegisterResult { id, rx, grew }
    }

    /// Remove interest in the mask from the subscription with `id`.
    ///
    /// Returns whether the union of all interest masks shrunk by removing this interest.
    pub fn unregister(&self, id: SubscriptionId) -> bool {
        let mut state = self.state.lock().expect("poisoned mutex");

        let Some(pos) = state
            .subscribers
            .iter()
            .position(|(sid, _mask, _tx)| *sid == id)
        else {
            return false;
        };

        let (_, mask, _) = state.subscribers.swap_remove(pos);

        let mut shrank = false;
        for interest in mask {
            if let Some(count) = state.interest_count.get_mut(&interest) {
                *count = count.saturating_sub(1);

                if *count == 0 {
                    shrank = true;
                }
            }
        }

        shrank
    }

    /// The union of all registered interest mask sets.
    pub fn current_mask(&self) -> InterestMaskSet {
        let state = self.state.lock().expect("poisoned mutex");

        InterestMaskSetBuilder::new()
            .set_all(
                state
                    .interest_count
                    .iter()
                    .filter(|(_mask, count)| **count > 0)
                    .map(|(mask, _count)| *mask),
            )
            .build()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, thread};

    use futures::channel::mpsc::TryRecvError;
    use libpulse_binding::context::subscribe::{Facility, Operation};

    use super::*;

    fn sink_mask() -> InterestMaskSet {
        InterestMaskSetBuilder::new()
            .set(InterestMask::Sink)
            .build()
    }

    fn source_mask() -> InterestMaskSet {
        InterestMaskSetBuilder::new()
            .set(InterestMask::Source)
            .build()
    }

    fn source_sink_mask() -> InterestMaskSet {
        InterestMaskSetBuilder::new()
            .set(InterestMask::Sink)
            .set(InterestMask::Source)
            .build()
    }

    fn subscription_event(facility: Facility) -> SubscriptionEvent {
        SubscriptionEvent {
            facility: facility,
            operation: Operation::New,
            index: 0,
        }
    }

    fn dispatch_all(dispatch: &SubDispatcher) {
        dispatch.dispatch(subscription_event(Facility::Card));
        dispatch.dispatch(subscription_event(Facility::Client));
        dispatch.dispatch(subscription_event(Facility::Module));
        dispatch.dispatch(subscription_event(Facility::SampleCache));
        dispatch.dispatch(subscription_event(Facility::Server));
        dispatch.dispatch(subscription_event(Facility::Sink));
        dispatch.dispatch(subscription_event(Facility::SinkInput));
        dispatch.dispatch(subscription_event(Facility::Source));
        dispatch.dispatch(subscription_event(Facility::SourceOutput));
    }

    #[test]
    fn test_register_ids() {
        let dispatch = SubDispatcher::new();

        let sub1 = dispatch.register(InterestMaskSet::none());
        let sub2 = dispatch.register(InterestMaskSet::none());
        let sub3 = dispatch.register(InterestMaskSet::none());

        assert!(sub1.id != sub2.id);
        assert!(sub2.id != sub3.id);
        assert!(sub1.id < sub2.id);
        assert!(sub2.id < sub3.id);
    }

    #[test]
    fn test_register_grow_sequential() {
        let dispatch = SubDispatcher::new();

        let sub1 = dispatch.register(sink_mask());
        assert!(sub1.grew);

        let sub2 = dispatch.register(source_mask());
        assert!(sub2.grew);

        let sub3 = dispatch.register(source_mask());
        assert!(!sub3.grew);
    }

    #[test]
    fn test_register_grow_partial_overlap() {
        let dispatch = SubDispatcher::new();

        let _sub1 = dispatch.register(sink_mask());
        let sub2 = dispatch.register(source_sink_mask());
        assert!(sub2.grew);
    }

    #[test]
    fn test_unregister_unknown_id() {
        let dispatch = SubDispatcher::new();

        assert!(
            !dispatch.unregister(1),
            "Unregistering an unknown sub should not shrink the mask"
        );
    }

    #[test]
    fn test_unregister_double() {
        let dispatch = SubDispatcher::new();

        let sub1 = dispatch.register(sink_mask());
        assert!(dispatch.unregister(sub1.id));
        assert!(
            !dispatch.unregister(sub1.id),
            "Double unregisters should not shrink the mask"
        );
    }

    #[test]
    fn test_unregister_shared_interest() {
        let dispatch = SubDispatcher::new();

        let sub1 = dispatch.register(sink_mask());
        let sub2 = dispatch.register(sink_mask());
        assert!(
            !dispatch.unregister(sub1.id),
            "Unregistering a mask with duplicate interest as an active sub should not shrink the mask"
        );
        assert!(
            dispatch.unregister(sub2.id),
            "Unregistering the last sub with a mask should shrink the mask"
        );
    }

    #[test]
    fn test_unregister_disjoint_interest() {
        let dispatch = SubDispatcher::new();

        let sub1 = dispatch.register(sink_mask());
        let _sub2 = dispatch.register(source_mask());
        assert!(
            dispatch.unregister(sub1.id),
            "Unregistering disjoint subs should cause the mask to shrink"
        );
    }

    #[test]
    fn test_post_unregister_dispatch() {
        let dispatch = SubDispatcher::new();

        let mut sub1 = dispatch.register(sink_mask());
        let mut sub2 = dispatch.register(sink_mask());
        let mut sub3 = dispatch.register(sink_mask());

        dispatch.unregister(sub2.id);
        dispatch.dispatch(subscription_event(Facility::Sink));

        assert_eq!(
            sub1.rx.try_recv().unwrap(),
            subscription_event(Facility::Sink),
            "The still-active subscription should recieve the event"
        );
        assert_eq!(
            sub2.rx.try_recv().unwrap_err(),
            TryRecvError::Closed,
            "The unregistered subscription should be closed"
        );
        assert_eq!(
            sub3.rx.try_recv().unwrap(),
            subscription_event(Facility::Sink),
            "The still-active subscription should receive the event"
        )
    }

    #[test]
    fn test_dispatch_no_interest() {
        let dispatch = SubDispatcher::new();

        let mut sub = dispatch.register(InterestMaskSet::none());

        dispatch_all(&dispatch);

        assert_eq!(
            sub.rx.try_recv().unwrap_err(),
            TryRecvError::Empty,
            "Sub with empty interest should not receive any events"
        );
    }

    #[test]
    fn test_dispatch_filters_by_interest() {
        let dispatch = SubDispatcher::new();

        let mut sub = dispatch.register(sink_mask());

        dispatch.dispatch(subscription_event(Facility::Sink));
        assert_eq!(
            sub.rx.try_recv().unwrap(),
            subscription_event(Facility::Sink),
            "Sub should receieve events it's subscribed to"
        );

        dispatch.dispatch(subscription_event(Facility::Source));
        assert_eq!(
            sub.rx.try_recv().unwrap_err(),
            TryRecvError::Empty,
            "Sub shouldn't receive events it isn't subscribed to"
        );
    }

    #[test]
    fn test_dispatch_independent_subs() {
        let dispatch = SubDispatcher::new();

        let mut sub1 = dispatch.register(sink_mask());
        let mut sub2 = dispatch.register(sink_mask());

        dispatch.dispatch(subscription_event(Facility::Sink));
        sub1.rx.try_recv().unwrap();
        assert_eq!(
            sub2.rx.try_recv().unwrap(),
            subscription_event(Facility::Sink),
            "Independent subs should receive events independently"
        );
    }

    #[test]
    fn test_dispatch_full_channel_doesnt_panic() {
        let dispatch = SubDispatcher::new();
        let _sub = dispatch.register(sink_mask());
        for _ in 0..CHANNEL_BOUND {
            dispatch.dispatch(subscription_event(Facility::Sink));
        }

        // This shouldn't panic or block
        dispatch.dispatch(subscription_event(Facility::Sink));
    }

    #[test]
    fn test_current_mask() {
        let dispatch = SubDispatcher::new();
        assert_eq!(
            dispatch.current_mask(),
            InterestMaskSet::none(),
            "Empty dispatcher should have an empty mask"
        );

        let sub1 = dispatch.register(sink_mask());
        let sub2 = dispatch.register(source_mask());
        assert_eq!(
            dispatch.current_mask(),
            source_sink_mask(),
            "Dispatcher with disjoint subscription interest should have the union of those masks"
        );

        let sub3 = dispatch.register(source_sink_mask());
        assert_eq!(
            dispatch.current_mask(),
            source_sink_mask(),
            "Dispatcher with overlapping subscription interests should have the union of those masks"
        );

        dispatch.unregister(sub1.id);
        assert_eq!(
            dispatch.current_mask(),
            source_sink_mask(),
            "Removing a sub with entirely overlapping interests shouldn't affect mask"
        );

        dispatch.unregister(sub3.id);
        assert_eq!(
            dispatch.current_mask(),
            source_mask(),
            "Removing a sub with partially overlapping interests should only affect non-overlapping interests"
        );

        let sub4 = dispatch.register(sink_mask());
        dispatch.unregister(sub4.id);
        assert_eq!(
            dispatch.current_mask(),
            source_mask(),
            "Removing a subscription with disjoin interest should remove it's interests"
        );

        dispatch.unregister(sub2.id);
        assert_eq!(
            dispatch.current_mask(),
            InterestMaskSet::none(),
            "Removing all subs from a dispatcher should leave an empty mask"
        );
    }

    #[test]
    fn test_concurrent() {
        let dispatch = SubDispatcher::new();
        let n = 100;

        let handles: Vec<_> = (0..n)
            .map(|_| {
                let dispatch = dispatch.clone();
                thread::spawn(move || {
                    let mask = InterestMaskSet::none();
                    dispatch.register(mask).id
                })
            })
            .collect();

        let ids: Vec<SubscriptionId> = handles
            .into_iter()
            .map(|h| h.join().expect("thread panicked"))
            .collect();

        assert_eq!(ids.len(), n);

        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), n, "duplicate ids found: {ids:?}")
    }
}
