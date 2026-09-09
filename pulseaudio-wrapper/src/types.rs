//! Owned, `'static`, `Send` snapshots of the borrowed introspection structs that `libpulse_binding` hands to callbacks.
//!
//! The borrowed types (e.g. `introspect::SinkInfo<'_>`) are only valid for the duration of the callback that produced
//! them and are tied to the worker thread that owns the `Context`. These types convert from inside the callback,
//! before the data crosses the channel into the async task.

/// Re-export of [`libpulse_binding::channelmap::Map`].
pub use libpulse_binding::channelmap::Map as ChannelMap;
/// Re-export of [`libpulse_binding::def::PortAvailable`]
pub use libpulse_binding::def::PortAvailable;
/// Re-export of [`libpulse_binding::def::SinkFlagSet`]
pub use libpulse_binding::def::SinkFlagSet;
/// Re-export of [`libpulse_binding::def::SinkState`]
pub use libpulse_binding::def::SinkState;
/// Re-export of [`libpulse_binding::def::SourceFlagSet`]
pub use libpulse_binding::def::SourceFlagSet;
/// Re-export of [`libpulse_binding::def::SourceState`]
pub use libpulse_binding::def::SourceState;
/// Re-export of [`libpulse_binding::sample::Spec`].
pub use libpulse_binding::sample::Spec as SampleSpec;
/// Re-export of [`libpulse_binding::time::MicroSeconds`].
pub use libpulse_binding::time::MicroSeconds;
/// Re-export of [`libpulse_binding::volume::ChannelVolumes`].
pub use libpulse_binding::volume::ChannelVolumes;
/// Re-export of [`libpulse_binding::volume::Volume`].
pub use libpulse_binding::volume::Volume;

use std::collections::HashMap;

use libpulse_binding::{context::introspect, proplist::Proplist};

/// Turns a property list into a plain owned map. Non-UTF8 values are dropped rather than exposing `libpulse`'s
/// borrowed/raw representation to callers.
fn proplist_to_hashmap(list: &Proplist) -> HashMap<Box<str>, Box<str>> {
    list.iter()
        .filter_map(|key| {
            let value = list.get_str(&key);
            value.map(|value| (key.into_boxed_str(), value.into_boxed_str()))
        })
        .collect()
}

/// A snapshot of a PulseAudio sink.
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct SinkInfo {
    /// The sink's numeric index. Stable for the lifetime of the sink.
    pub index: u32,
    /// The sink's short, stable name (e.g. `alsa_output.pci-0000_29_00.1.hdmi-stereo`).
    pub name: Option<Box<str>>,
    /// The sink's human-readable description.
    pub description: Option<Box<str>>,
    /// The sample format, rate, and channel count the sink is running at.
    pub sample_spec: SampleSpec,
    /// The mapping from channel index to speaker position.
    pub channel_map: ChannelMap,
    /// Index of the module that owns this sink, if any.
    pub owner_module: Option<u32>,
    /// Per-channel volume.
    pub volume: ChannelVolumes,
    /// Whether the sink is muted.
    pub mute: bool,
    /// Index of the monitor source that captures this sink's output.
    pub monitor_source: u32,
    /// Name of the monitor source that captures this sink's output.
    pub monitor_source_name: Option<Box<str>>,
    /// Length of the audio queued in the sink's output buffer.
    pub latency: MicroSeconds,
    /// Name of the driver backing this sink.
    pub driver: Option<Box<str>>,
    /// Sink capability flags
    pub flags: SinkFlagSet,
    /// The sink's property list, flattened to owned strings.
    pub properties: HashMap<Box<str>, Box<str>>,
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
            name: info.name.as_ref().map(|c| c.as_ref().into()),
            description: info.description.as_ref().map(|c| c.as_ref().into()),
            sample_spec: info.sample_spec,
            channel_map: info.channel_map,
            owner_module: info.owner_module,
            volume: info.volume,
            mute: info.mute,
            monitor_source: info.monitor_source,
            monitor_source_name: info.monitor_source_name.as_ref().map(|c| c.as_ref().into()),
            latency: info.latency,
            driver: info.driver.as_ref().map(|c| c.as_ref().into()),
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
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct DevicePort {
    /// The port's short, stable name.
    pub name: Option<Box<str>>,
    /// The port's human-readable description.
    pub description: Option<Box<str>>,
    /// The higher this value, the more sensible a default this port is.
    pub priority: u32,
    /// Whether the port is currently available.
    pub available: PortAvailable,
}

impl From<&introspect::SinkPortInfo<'_>> for DevicePort {
    fn from(port: &introspect::SinkPortInfo<'_>) -> Self {
        DevicePort {
            name: port.name.as_ref().map(|c| c.as_ref().into()),
            description: port.description.as_ref().map(|c| c.as_ref().into()),
            priority: port.priority,
            available: port.available,
        }
    }
}

impl From<&introspect::SourcePortInfo<'_>> for DevicePort {
    fn from(p: &introspect::SourcePortInfo<'_>) -> Self {
        DevicePort {
            name: p.name.as_ref().map(|c| c.as_ref().into()),
            description: p.description.as_ref().map(|c| c.as_ref().into()),
            priority: p.priority,
            available: p.available,
        }
    }
}

/// A snapshot of a PulseAudio source.
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// The source's numeric index. Stable for the lifetime of the source, reusable afterwards.
    pub index: u32,
    /// The sources's short, stable name.
    pub name: Option<Box<str>>,
    /// The source's human-readable description
    pub description: Option<Box<str>>,
    /// The sample format, rate, and channel count the source is running at.
    pub sample_spec: SampleSpec,
    /// The mapping from channel index to microphone/position.
    pub channel_map: ChannelMap,
    /// Index of the module that owns this source, if any.
    pub owner_module: Option<u32>,
    /// Per-channel volume.
    pub volume: ChannelVolumes,
    /// Whether the source is muted.
    pub mute: bool,
    /// If this is a monitor source, the index of the sink it monitors.
    pub monitor_of_sink: Option<u32>,
    /// If this is a monitor source, the name of the sink it monitors.
    pub monitor_of_sink_name: Option<Box<str>>,
    /// Length of the source's filled record buffer.
    pub latency: MicroSeconds,
    /// Name of the driver backing this source.
    pub driver: Option<Box<str>>,
    /// Source capability flags.
    pub flags: SourceFlagSet,
    /// the source's property list, flattened to owned strings.
    pub properties: HashMap<Box<str>, Box<str>>,
    /// The latency the source has actually been configured to.
    pub configured_latency: MicroSeconds,
    /// the "base" (unamplified/unattenuated) volume of the source.
    pub base_volume: Volume,
    /// The source's current running state.
    pub state: SourceState,
    /// Number of discrete volume steps, for sources that don't support arbitrary volumes.
    pub n_volume_steps: u32,
    /// Index of the card that owns this source, if any.
    pub card: Option<u32>,
    /// The set of ports available on this source.
    pub ports: Vec<DevicePort>,
    /// The currently active port, if any.
    pub active_port: Option<DevicePort>,
}

impl From<&introspect::SourceInfo<'_>> for SourceInfo {
    fn from(info: &introspect::SourceInfo<'_>) -> Self {
        SourceInfo {
            index: info.index,
            name: info.name.as_ref().map(|c| c.as_ref().into()),
            description: info.description.as_ref().map(|c| c.as_ref().into()),
            sample_spec: info.sample_spec,
            channel_map: info.channel_map,
            owner_module: info.owner_module,
            volume: info.volume,
            mute: info.mute,
            monitor_of_sink: info.monitor_of_sink,
            monitor_of_sink_name: info
                .monitor_of_sink_name
                .as_ref()
                .map(|c| c.as_ref().into()),
            latency: info.latency,
            driver: info.driver.as_ref().map(|c| c.as_ref().into()),
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
