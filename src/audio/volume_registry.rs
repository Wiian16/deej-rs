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
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VolumeRegistryInner::new())),
        }
    }

    /// Resolve a volume target into a registered volume. Fall back to unmapped volume for processes if set. Returns
    ///
    /// For processes: return None if there is no volume set for the process and there is no unmapped volume set. For
    /// other types: return None if there is no volume set.
    pub fn resolve(&self, target: &VolumeTarget) -> Option<NormalizedVolume> {
        self.inner.lock().expect("poisoned mutex").resolve(target)
    }

    /// Resolve a process volume, falling back to unmapped if it isn't registered.
    pub fn resolve_process(&self, process: &str) -> Option<NormalizedVolume> {
        self.inner
            .lock()
            .expect("poisoned mutex")
            .resolve_process(process)
    }

    /// Resolve a volume into a registered volume without falling back to unmapped for processes.
    ///
    /// If there is no registered volume for a process, `None` will be returned, not the value for unmapped.
    pub fn resolve_exact(&self, target: &VolumeTarget) -> Option<NormalizedVolume> {
        self.inner
            .lock()
            .expect("poisoned mutex")
            .resolve_exact(target)
    }

    /// Resolve a process volume, returns `None` if it isn't registered.
    pub fn resolve_process_exact(&self, process: &str) -> Option<NormalizedVolume> {
        self.inner
            .lock()
            .expect("poisoned mutex")
            .resolve_process_exact(process)
    }

    /// Register a volume with the registry.
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
    process_map: HashMap<String, NormalizedVolume>,
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
        match target {
            &VolumeTarget::Master => self.master,
            &VolumeTarget::Mic => self.mic,
            &VolumeTarget::Process(ref name) => self.resolve_process(name),
            &VolumeTarget::Unmapped => self.unmapped,
        }
    }

    /// Resolve a process volume, falling back to unmapped if it isn't registered.
    fn resolve_process(&self, process: &str) -> Option<NormalizedVolume> {
        let mapped = self.process_map.get(process).cloned();

        if mapped.is_some() {
            return mapped;
        }

        self.unmapped
    }

    /// Resolve a volume into a registered volume without falling back to unmapped for processes.
    ///
    /// If there is no registered volume for a process, `None` will be returned, not the value for unmapped.
    fn resolve_exact(&self, target: &VolumeTarget) -> Option<NormalizedVolume> {
        match target {
            &VolumeTarget::Master => self.master,
            &VolumeTarget::Mic => self.mic,
            &VolumeTarget::Process(ref name) => self.resolve_process_exact(name),
            &VolumeTarget::Unmapped => self.unmapped,
        }
    }

    /// Resolve a process volume, returns `None` if it isn't registered.
    fn resolve_process_exact(&self, process: &str) -> Option<NormalizedVolume> {
        self.process_map.get(process).cloned()
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
        };
    }
}

impl Default for VolumeRegistryInner {
    fn default() -> Self {
        Self::new()
    }
}
