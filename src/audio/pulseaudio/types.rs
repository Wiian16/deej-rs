//! Owned, `'static`, `Send` snapshots of the borrowed introspection structs that `libpulse_binding` hands to callbacks.
//!
//! The borrowed types (e.g. `introspect::SinkInfo<'_>`) are only valid for the duration of the callback that produced
//! them and are tied to the worker thread that owns the `Context`. These types convert from inside the callback,
//! before the data crosses the channel into the async task.

use std::{collections::HashMap, fmt::DebugStruct};

use libpulse_binding::{
    channelmap,
    context::introspect,
    def::{PortAvailable, SinkFlagSet, SinkState},
    proplist::Proplist,
    sample,
    time::MicroSeconds,
    volume::{ChannelVolumes, Volume},
};

/// Turns a property list into a plain owned map. Non-UTF8 values are dropped rather than exposing `libpulse`'s
/// borrowed/raw representation to callers.
fn proplist_to_hashmap(list: &Proplist) -> HashMap<String, String> {
    list.iter()
        .filter_map(|key| {
            let value = list.get_str(&key);
            value.map(|value| (key, value))
        })
        .collect()
}

/// A snapshot of a PulseAudio sink.
#[derive(Debug, Clone)]
pub struct SinkInfo {
    /// The sink's numeric index. Stable for the lifetime of the sink.
    pub index: u32,
    /// The sink's short, stable name (e.g. `alsa_output.pci-0000_29_00.1.hdmi-stereo`).
    pub name: Option<String>,
    /// The sink's human-readable description.
    pub description: Option<String>,
    /// The sample format, rate, and channel count the sink is running at.
    pub sample_spec: sample::Spec,
    /// The mapping from channel index to speaker position.
    pub channel_map: channelmap::Map,
    /// Index of the module that owns this sink, if any.
    pub owner_module: Option<u32>,
    /// Per-channel volume.
    pub volume: ChannelVolumes,
    /// Whether the sink is muted.
    pub mute: bool,
    /// Index of the monitor source that captures this sink's output.
    pub monitor_source: u32,
    /// Name of the monitor source that captures this sink's output.
    pub monitor_source_name: Option<String>,
    /// Length of the audio queued in the sink's output buffer.
    pub latency: MicroSeconds,
    /// Name of the driver backing this sink.
    pub driver: Option<String>,
    /// Sink capability flags
    pub flags: SinkFlagSet,
    /// The sink's property list, flattened to owned strings.
    pub properties: HashMap<String, String>,
    /// The latency the sink has actually been configured to.
    pub configured_latency: MicroSeconds,
    /// the "base" (unamplified/unattenuated) volume of the sink.
    pub base_volume: Volume,
    /// The sink's current running state.
    pub state: SinkState,
    /// Number of discrete steps, for sinks that don't support arbitrary volumes.
    pub n_volume_steps: u32,
    /// Index of the card that owns this sink, if any.
    pub card: Option<u32>,
    /// The set of ports available on this sink.
    pub ports: Vec<DevicePort>,
    /// The currently active port, if any.
    pub active_port: Option<DevicePort>,
}

impl From<&introspect::SinkInfo<'_>> for SinkInfo {
    fn from(info: &introspect::SinkInfo<'_>) -> Self {
        SinkInfo {
            index: info.index,
            name: info.name.as_ref().map(|c| c.to_string()),
            description: info.description.as_ref().map(|c| c.to_string()),
            sample_spec: info.sample_spec,
            channel_map: info.channel_map,
            owner_module: info.owner_module,
            volume: info.volume,
            mute: info.mute,
            monitor_source: info.monitor_source,
            monitor_source_name: info.monitor_source_name.as_ref().map(|c| c.to_string()),
            latency: info.latency,
            driver: info.driver.as_ref().map(|c| c.to_string()),
            flags: info.flags,
            properties: proplist_to_hashmap(&info.proplist),
            configured_latency: info.configured_latency,
            base_volume: info.base_volume,
            state: info.state,
            n_volume_steps: info.n_volume_steps,
            card: info.card,
            ports: info.ports.iter().map(DevicePort::from).collect(),
            active_port: info.active_port.as_deref().map(DevicePort::from),
        }
    }
}

/// A port belonging to a [`SinkInfo`] or [`SourceInfo`].
///
/// `libpulse_binding` models sink ports and source ports as two distinct (but identical) types. This crate merges them.
#[derive(Debug, Clone)]
pub struct DevicePort {
    /// The port's short, stable name.
    pub name: Option<String>,
    /// The port's human-readable description.
    pub description: Option<String>,
    /// The higher this value, the more sensible a default this port is.
    pub priority: u32,
    /// Whether the port is currently available.
    pub available: PortAvailable,
}

impl From<&introspect::SinkPortInfo<'_>> for DevicePort {
    fn from(port: &introspect::SinkPortInfo<'_>) -> Self {
        DevicePort {
            name: port.name.as_ref().map(|c| c.to_string()),
            description: port.description.as_ref().map(|c| c.to_string()),
            priority: port.priority,
            available: port.available,
        }
    }
}
