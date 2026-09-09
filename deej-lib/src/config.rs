use std::collections::HashMap;

use crate::audio::VolumeTarget;

/// Fully resolved service config. This has no notion of files, YAML, or
/// magic strings.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub slider_mapping: HashMap<u8, Vec<VolumeTarget>>,
    pub invert_sliders: bool,
    pub com_port: String,
    pub baud_rate: u32,
    pub noise_reduction: NoiseReduction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseReduction {
    Low,
    Default,
    High,
}
