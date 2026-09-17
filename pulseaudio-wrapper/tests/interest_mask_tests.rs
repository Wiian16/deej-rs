//! Integration tests for `pulseaudio_wrapper::types`.
//!
//! Place this file at `tests/interest_mask.rs` in the crate so it's compiled
//! as a separate integration test binary against the public API.

use pulseaudio_wrapper::types::{InterestMask, InterestMaskSet, InterestMaskSetBuilder};

#[test]
fn none_contains_nothing() {
    let set = InterestMaskSet::none();
    for interest in InterestMaskSet::all() {
        assert!(!set.contains(interest));
    }
    assert_eq!(set.iter().count(), 0);
}

#[test]
fn all_contains_everything() {
    let set = InterestMaskSet::all();
    let all_variants = [
        InterestMask::Sink,
        InterestMask::Source,
        InterestMask::SinkInput,
        InterestMask::SourceOutput,
        InterestMask::Module,
        InterestMask::Client,
        InterestMask::SampleCache,
        InterestMask::Server,
        InterestMask::Card,
    ];

    for interest in all_variants {
        assert!(set.contains(interest));
    }
    assert_eq!(set.iter().count(), all_variants.len());
}

#[test]
fn default_is_none() {
    assert_eq!(InterestMaskSet::default(), InterestMaskSet::none());
}

#[test]
fn builder_set_and_unset() {
    let set = InterestMaskSetBuilder::new()
        .set(InterestMask::Sink)
        .set(InterestMask::Card)
        .build();

    assert!(set.contains(InterestMask::Sink));
    assert!(set.contains(InterestMask::Card));
    assert!(!set.contains(InterestMask::Source));

    let set = InterestMaskSetBuilder::new()
        .set_all([InterestMask::Sink, InterestMask::Card])
        .build();
    assert!(set.contains(InterestMask::Sink));
    assert!(set.contains(InterestMask::Card));

    let unset = InterestMaskSetBuilder::new()
        .set_all([InterestMask::Sink, InterestMask::Card])
        .unset(InterestMask::Sink)
        .build();
    assert!(!unset.contains(InterestMask::Sink));
    assert!(unset.contains(InterestMask::Card));

    let unset_all = InterestMaskSetBuilder::new()
        .set_all(InterestMaskSet::all())
        .unset_all([InterestMask::Sink, InterestMask::Source])
        .build();
    assert!(!unset_all.contains(InterestMask::Sink));
    assert!(!unset_all.contains(InterestMask::Source));
    assert!(unset_all.contains(InterestMask::Card));
}

#[test]
fn bitor_two_masks_builds_set() {
    let set = InterestMask::Sink | InterestMask::Source;
    assert!(set.contains(InterestMask::Sink));
    assert!(set.contains(InterestMask::Source));
    assert!(!set.contains(InterestMask::Card));
}

#[test]
fn bitor_set_with_mask_adds_entry() {
    let set = InterestMaskSet::none() | InterestMask::Module;
    assert!(set.contains(InterestMask::Module));
    assert!(!set.contains(InterestMask::Client));

    let set = set | InterestMask::Client;
    assert!(set.contains(InterestMask::Module));
    assert!(set.contains(InterestMask::Client));
}

#[test]
fn iter_yields_only_set_members_in_order() {
    let set = InterestMaskSetBuilder::new()
        .set(InterestMask::Source)
        .set(InterestMask::Server)
        .build();

    let members: Vec<_> = set.iter().collect();
    assert_eq!(members, vec![InterestMask::Source, InterestMask::Server]);
}

#[test]
fn iter_by_ref_matches_owned_iter() {
    let set = InterestMaskSetBuilder::new()
        .set(InterestMask::Sink)
        .set(InterestMask::Client)
        .build();

    let via_ref: Vec<_> = (&set).into_iter().collect();
    let via_owned: Vec<_> = set.into_iter().collect();
    assert_eq!(via_ref, via_owned);
}

#[test]
fn size_hint_is_consistent_with_remaining_items() {
    let set = InterestMaskSet::all();
    let mut iter = set.iter();
    let total = 9;

    for expected_remaining in (0..=total).rev() {
        let (_, upper) = iter.size_hint();
        assert_eq!(upper, Some(expected_remaining));
        iter.next();
    }
}

#[test]
fn round_trip_through_raw_interest_mask_set() {
    use libpulse_binding::context::subscribe::InterestMaskSet as Raw;

    let set = InterestMaskSetBuilder::new()
        .set(InterestMask::Sink)
        .set(InterestMask::SinkInput)
        .set(InterestMask::Card)
        .build();

    let raw: Raw = set.into();
    assert!(raw.contains(Raw::SINK));
    assert!(raw.contains(Raw::SINK_INPUT));
    assert!(raw.contains(Raw::CARD));
    assert!(!raw.contains(Raw::SOURCE));

    let back: InterestMaskSet = raw.into();
    assert_eq!(back, set);
}

#[test]
fn single_mask_converts_to_matching_raw_flag() {
    use libpulse_binding::context::subscribe::InterestMaskSet as Raw;

    let raw: Raw = InterestMask::Server.into();
    assert_eq!(raw, Raw::SERVER);
}

#[test]
fn empty_and_full_sets_round_trip() {
    use libpulse_binding::context::subscribe::InterestMaskSet as Raw;

    let none = InterestMaskSet::none();
    let raw_none: Raw = none.into();
    let back_none: InterestMaskSet = raw_none.into();
    assert_eq!(back_none, none);

    let all = InterestMaskSet::all();
    let raw_all: Raw = all.into();
    let back_all: InterestMaskSet = raw_all.into();
    assert_eq!(back_all, all);
}
