use std::cell::LazyCell;

use deej_rs::audio::{NormalizedVolume, VolumeTarget, volume_registry::VolumeRegistry};

const PROCESS1: &'static str = "process1";
const PROCESS2: &'static str = "process2";

const MASTER_VOLUME: LazyCell<NormalizedVolume> = LazyCell::new(|| NormalizedVolume::clamped(0.0));
const MIC_VOLUME: LazyCell<NormalizedVolume> = LazyCell::new(|| NormalizedVolume::clamped(0.1));
const UNMAPPED_VOLUME: LazyCell<NormalizedVolume> =
    LazyCell::new(|| NormalizedVolume::clamped(0.2));
const PROCESS1_VOLUME: LazyCell<NormalizedVolume> =
    LazyCell::new(|| NormalizedVolume::clamped(0.3));
const PROCESS2_VOLUME: LazyCell<NormalizedVolume> =
    LazyCell::new(|| NormalizedVolume::clamped(0.4));

/// Creates a volume registry with the mapping:
///     Master -> 0.0
///     Mic -> 0.1
///     Unmapped -> 0.2
///     'process1' -> 0.3
///     'process2' -> 0.4
fn testing_registry() -> VolumeRegistry {
    let registry = VolumeRegistry::new();

    registry.register(VolumeTarget::Master, *MASTER_VOLUME);
    registry.register(VolumeTarget::Mic, *MIC_VOLUME);
    registry.register(VolumeTarget::Unmapped, *UNMAPPED_VOLUME);
    registry.register(VolumeTarget::Process(PROCESS1.into()), *PROCESS1_VOLUME);
    registry.register(VolumeTarget::Process(PROCESS2.into()), *PROCESS2_VOLUME);

    registry
}

#[test]
fn test_resolve_master() {
    let empty_registry = VolumeRegistry::new();
    let filled_registry = testing_registry();

    assert_eq!(empty_registry.resolve(&VolumeTarget::Master), None);
    assert_eq!(
        filled_registry.resolve(&VolumeTarget::Master),
        Some(*MASTER_VOLUME)
    );
}

#[test]
fn test_resolve_exact_master() {
    let empty_registry = VolumeRegistry::new();
    let filled_registry = testing_registry();

    assert_eq!(empty_registry.resolve_exact(&VolumeTarget::Master), None);
    assert_eq!(
        filled_registry.resolve_exact(&VolumeTarget::Master),
        Some(*MASTER_VOLUME)
    );
}

#[test]
fn test_resolve_mic() {
    let empty_registry = VolumeRegistry::new();
    let filled_registry = testing_registry();

    assert_eq!(empty_registry.resolve(&VolumeTarget::Mic), None);
    assert_eq!(
        filled_registry.resolve(&VolumeTarget::Mic),
        Some(*MIC_VOLUME)
    );
}

#[test]
fn test_resolve_exact_mic() {
    let empty_registry = VolumeRegistry::new();
    let filled_registry = testing_registry();

    assert_eq!(empty_registry.resolve_exact(&VolumeTarget::Mic), None);
    assert_eq!(
        filled_registry.resolve_exact(&VolumeTarget::Mic),
        Some(*MIC_VOLUME)
    );
}

#[test]
fn test_resolve_unmapped() {
    let empty_registry = VolumeRegistry::new();
    let filled_registry = testing_registry();

    assert_eq!(empty_registry.resolve(&VolumeTarget::Unmapped), None);
    assert_eq!(
        filled_registry.resolve(&VolumeTarget::Unmapped),
        Some(*UNMAPPED_VOLUME)
    );
}

#[test]
fn test_resolve_exact_unmapped() {
    let empty_registry = VolumeRegistry::new();
    let filled_registry = testing_registry();

    assert_eq!(empty_registry.resolve_exact(&VolumeTarget::Unmapped), None);
    assert_eq!(
        filled_registry.resolve_exact(&VolumeTarget::Unmapped),
        Some(*UNMAPPED_VOLUME)
    );
}

#[test]
fn test_resolve_process() {
    let registry = testing_registry();

    assert_eq!(
        registry.resolve(&VolumeTarget::Process(PROCESS1.into())),
        Some(*PROCESS1_VOLUME)
    );
    assert_eq!(
        registry.resolve(&VolumeTarget::Process(PROCESS2.into())),
        Some(*PROCESS2_VOLUME)
    );
    assert_eq!(registry.resolve_process(PROCESS1), Some(*PROCESS1_VOLUME));
    assert_eq!(registry.resolve_process(PROCESS2), Some(*PROCESS2_VOLUME));
}

#[test]
fn test_resolve_unmapped_process() {
    let registry = VolumeRegistry::new();

    assert_eq!(
        registry.resolve(&VolumeTarget::Process(PROCESS1.into())),
        None
    );
    assert_eq!(
        registry.resolve(&VolumeTarget::Process(PROCESS2.into())),
        None
    );
    assert_eq!(registry.resolve_process(PROCESS1), None);
    assert_eq!(registry.resolve_process(PROCESS2), None);

    registry.register(VolumeTarget::Unmapped, *UNMAPPED_VOLUME);

    assert_eq!(
        registry.resolve(&VolumeTarget::Process(PROCESS1.into())),
        Some(*UNMAPPED_VOLUME)
    );
    assert_eq!(
        registry.resolve(&VolumeTarget::Process(PROCESS2.into())),
        Some(*UNMAPPED_VOLUME)
    );
    assert_eq!(registry.resolve_process(PROCESS1), Some(*UNMAPPED_VOLUME));
    assert_eq!(registry.resolve_process(PROCESS2), Some(*UNMAPPED_VOLUME));
}

#[test]
fn test_resolve_exact_unmapped_process() {
    let registry = VolumeRegistry::new();

    assert_eq!(
        registry.resolve_exact(&VolumeTarget::Process(PROCESS1.into())),
        None
    );
    assert_eq!(
        registry.resolve_exact(&VolumeTarget::Process(PROCESS2.into())),
        None
    );
    assert_eq!(registry.resolve_process_exact(PROCESS1), None);
    assert_eq!(registry.resolve_process_exact(PROCESS2), None);

    registry.register(VolumeTarget::Unmapped, *UNMAPPED_VOLUME);

    assert_eq!(
        registry.resolve_exact(&VolumeTarget::Process(PROCESS1.into())),
        None
    );
    assert_eq!(
        registry.resolve_exact(&VolumeTarget::Process(PROCESS2.into())),
        None
    );
    assert_eq!(registry.resolve_process_exact(PROCESS1), None);
    assert_eq!(registry.resolve_process_exact(PROCESS2), None);
}
