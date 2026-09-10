use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use crate::audio::{NormalizedVolume, VolumeTarget};

/// A thread-safe handle to a volume registry, capable of registering and resolving volume targets and their volumes.
///
/// To share between threads, you may clone the registry, which clones a reference to the underlying inner registry.
/// Accesses to this registry are locked to remain thread-safe. The underlying registry will be dropped when the last
/// handle is dropped.
#[derive(Clone)]
pub struct VolumeRegistry {
    inner: Arc<Mutex<VolumeRegistryInner>>,
}

impl VolumeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VolumeRegistryInner::new())),
        }
    }

    /// Resolve a volume target into a registered volume. Fall back to unmapped volume for processes if set. Returns
    ///
    /// For processes: return None if there is no volume set for the process and there is no unmapped volume set. For
    /// other types: return None if there is no volume set.
    ///
    /// # Panics
    ///
    /// Will panic if the internal mutex is poisoned. See [poisoning](Mutex#poisoning).
    #[must_use]
    pub fn resolve(&self, target: &VolumeTarget) -> Option<NormalizedVolume> {
        self.inner.lock().expect("poisoned mutex").resolve(target)
    }

    /// Resolve a process volume, falling back to unmapped if it isn't registered.
    ///
    /// # Panics
    ///
    /// Will panic if the internal mutex is poisoned. See [poisoning](Mutex#poisoning).
    #[must_use]
    pub fn resolve_process(&self, process: &str) -> Option<NormalizedVolume> {
        self.inner
            .lock()
            .expect("poisoned mutex")
            .resolve_process(process)
    }

    /// Resolve a volume into a registered volume without falling back to unmapped for processes.
    ///
    /// If there is no registered volume for a process, `None` will be returned, not the value for unmapped.
    ///
    /// # Panics
    ///
    /// Will panic if the internal mutex is poisoned. See [poisoning](Mutex#poisoning).
    #[must_use]
    pub fn resolve_exact(&self, target: &VolumeTarget) -> Option<NormalizedVolume> {
        self.inner
            .lock()
            .expect("poisoned mutex")
            .resolve_exact(target)
    }

    /// Resolve a process volume, returns `None` if it isn't registered.
    ///
    /// # Panics
    ///
    /// Will panic if the internal mutex is poisoned. See [poisoning](Mutex#poisoning).
    #[must_use]
    pub fn resolve_process_exact(&self, process: &str) -> Option<NormalizedVolume> {
        self.inner
            .lock()
            .expect("poisoned mutex")
            .resolve_process_exact(process)
    }

    /// Register a volume with the registry.
    ///
    /// # Panics
    ///
    /// Will panic if the internal mutex is poisoned. See [poisoning](Mutex#poisoning).
    pub fn register(&self, target: VolumeTarget, volume: NormalizedVolume) {
        self.inner
            .lock()
            .expect("poisoned mutex")
            .register(target, volume);
    }
}

impl Debug for VolumeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VolumeRegistry")
            .field("inner", &self.inner.lock().expect("poisoned mutex"))
            .finish()
    }
}

impl Default for VolumeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct VolumeRegistryInner {
    process_map: HashMap<Box<str>, NormalizedVolume>,
    master: Option<NormalizedVolume>,
    mic: Option<NormalizedVolume>,
    unmapped: Option<NormalizedVolume>,
}

impl VolumeRegistryInner {
    fn new() -> Self {
        Self {
            process_map: HashMap::new(),
            master: None,
            mic: None,
            unmapped: None,
        }
    }

    /// Resolve a volume target into a registered volume. Fall back to unmapped volume for processes if set. Returns
    ///
    /// For processes: return None if there is no volume set for the process and there is no unmapped volume set. For
    /// other types: return None if there is no volume set.
    fn resolve(&self, target: &VolumeTarget) -> Option<NormalizedVolume> {
        match *target {
            VolumeTarget::Master => self.master,
            VolumeTarget::Mic => self.mic,
            VolumeTarget::Process(ref name) => self.resolve_process(name),
            VolumeTarget::Unmapped => self.unmapped,
        }
    }

    /// Resolve a process volume, falling back to unmapped if it isn't registered.
    fn resolve_process(&self, process: &str) -> Option<NormalizedVolume> {
        let mapped = self.process_map.get(process).copied();

        if mapped.is_some() {
            return mapped;
        }

        self.unmapped
    }

    /// Resolve a volume into a registered volume without falling back to unmapped for processes.
    ///
    /// If there is no registered volume for a process, `None` will be returned, not the value for unmapped.
    fn resolve_exact(&self, target: &VolumeTarget) -> Option<NormalizedVolume> {
        match *target {
            VolumeTarget::Master => self.master,
            VolumeTarget::Mic => self.mic,
            VolumeTarget::Process(ref name) => self.resolve_process_exact(name),
            VolumeTarget::Unmapped => self.unmapped,
        }
    }

    /// Resolve a process volume, returns `None` if it isn't registered.
    fn resolve_process_exact(&self, process: &str) -> Option<NormalizedVolume> {
        self.process_map.get(process).copied()
    }

    /// Register a volume with the registry.
    fn register(&mut self, target: VolumeTarget, volume: NormalizedVolume) {
        match target {
            VolumeTarget::Master => self.master = Some(volume),
            VolumeTarget::Mic => self.mic = Some(volume),
            VolumeTarget::Process(name) => {
                let _ = self.process_map.insert(name, volume);
            }
            VolumeTarget::Unmapped => self.unmapped = Some(volume),
        }
    }
}

impl Default for VolumeRegistryInner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_master() {
        let mut registry = VolumeRegistryInner::new();

        assert_eq!(registry.master, None, "Master volume should start as None");

        registry.register(VolumeTarget::Master, NormalizedVolume::clamped(1.0));
        assert_eq!(
            registry.master,
            Some(NormalizedVolume::clamped(1.0)),
            "Master should be set to 1.0"
        );

        registry.register(VolumeTarget::Master, NormalizedVolume::clamped(0.0));
        assert_eq!(
            registry.master,
            Some(NormalizedVolume::clamped(0.0)),
            "Master should be set to 0.0"
        );
    }

    #[test]
    fn test_register_mic() {
        let mut registry = VolumeRegistryInner::new();

        assert_eq!(registry.mic, None, "Mic volume should start as None");

        registry.register(VolumeTarget::Mic, NormalizedVolume::clamped(1.0));
        assert_eq!(
            registry.mic,
            Some(NormalizedVolume::clamped(1.0)),
            "Mic should be set to 1.0"
        );

        registry.register(VolumeTarget::Mic, NormalizedVolume::clamped(0.0));
        assert_eq!(
            registry.mic,
            Some(NormalizedVolume::clamped(0.0)),
            "Mic should be set to 0.0"
        );
    }

    #[test]
    fn test_register_unmapped() {
        let mut registry = VolumeRegistryInner::new();

        assert_eq!(
            registry.unmapped, None,
            "Unmapped volume should start as None"
        );

        registry.register(VolumeTarget::Unmapped, NormalizedVolume::clamped(1.0));
        assert_eq!(
            registry.unmapped,
            Some(NormalizedVolume::clamped(1.0)),
            "Unmapped should be set to 1.0"
        );

        registry.register(VolumeTarget::Unmapped, NormalizedVolume::clamped(0.0));
        assert_eq!(
            registry.unmapped,
            Some(NormalizedVolume::clamped(0.0)),
            "Unmapped should be set to 0.0"
        );
    }

    #[test]
    fn test_register_process() {
        let mut registry = VolumeRegistryInner::new();

        assert_eq!(
            registry.process_map.len(),
            0,
            "Process map should start empty"
        );

        registry.register(
            VolumeTarget::Process("process1".into()),
            NormalizedVolume::clamped(1.0),
        );
        assert_eq!(
            registry.process_map.len(),
            1,
            "Process map should have 1 entry"
        );
        assert_eq!(
            registry.process_map.get("process1"),
            Some(&NormalizedVolume::clamped(1.0)),
            "'process1' should be set to 1.0"
        );

        registry.register(
            VolumeTarget::Process("process2".into()),
            NormalizedVolume::clamped(1.0),
        );
        assert_eq!(
            registry.process_map.len(),
            2,
            "Process map should have 2 entries"
        );
        assert_eq!(
            registry.process_map.get("process1"),
            Some(&NormalizedVolume::clamped(1.0)),
            "'process1' volume should not have changed"
        );
        assert_eq!(
            registry.process_map.get("process2"),
            Some(&NormalizedVolume::clamped(1.0)),
            "'process2' should be set to 1.0"
        );

        registry.register(
            VolumeTarget::Process("process2".into()),
            NormalizedVolume::clamped(0.0),
        );
        assert_eq!(
            registry.process_map.len(),
            2,
            "Process map should have 2 entries"
        );
        assert_eq!(
            registry.process_map.get("process1"),
            Some(&NormalizedVolume::clamped(1.0)),
            "'process1' volume should not have changed"
        );
        assert_eq!(
            registry.process_map.get("process2"),
            Some(&NormalizedVolume::clamped(0.0)),
            "'process2' should be set to 0.0"
        );
    }
}
