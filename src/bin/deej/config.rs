use core::fmt;
use std::{collections::HashMap, io};

use deej_rs::{
    audio::VolumeTarget,
    config::{NoiseReduction, ServiceConfig},
};
use serde::Deserialize;

pub fn load(path: impl AsRef<std::path::Path>) -> Result<ServiceConfig, ConfigError> {
    let contents = std::fs::read_to_string(path)?;
    let config: RawConfig = serde_saphyr::from_str(&contents)?;
    Ok(config.into())
}

#[derive(Debug, Deserialize)]
pub struct RawConfig {
    slider_mapping: HashMap<u8, RawSliderMapping>,
    invert_sliders: bool,
    com_port: String,
    baud_rate: u32,
    noise_reduction: RawNoiseReduction,
}

impl From<RawConfig> for ServiceConfig {
    fn from(raw: RawConfig) -> Self {
        Self {
            slider_mapping: raw
                .slider_mapping
                .into_iter()
                .map(|(idx, mapping)| (idx, mapping.into()))
                .collect(),
            invert_sliders: raw.invert_sliders,
            com_port: raw.com_port,
            baud_rate: raw.baud_rate,
            noise_reduction: raw.noise_reduction.into(),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawNoiseReduction {
    Low,
    Default,
    High,
}

impl From<RawNoiseReduction> for NoiseReduction {
    fn from(raw: RawNoiseReduction) -> Self {
        match raw {
            RawNoiseReduction::Low => Self::Low,
            RawNoiseReduction::Default => Self::Default,
            RawNoiseReduction::High => Self::High,
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum RawSliderMapping {
    Target(String),
    Targets(Vec<String>),
}

impl From<RawSliderMapping> for Vec<VolumeTarget> {
    fn from(raw: RawSliderMapping) -> Self {
        match raw {
            RawSliderMapping::Target(name) => vec![resolve_special(name)],
            RawSliderMapping::Targets(names) => names
                .into_iter()
                .map(|name| resolve_special(name))
                .collect(),
        }
    }
}

fn resolve_special(name: String) -> VolumeTarget {
    if name.eq_ignore_ascii_case("master") {
        VolumeTarget::Master
    } else if name.eq_ignore_ascii_case("mic") {
        VolumeTarget::Mic
    } else if name.eq_ignore_ascii_case("deej.unmapped") {
        VolumeTarget::Unmapped
    } else {
        VolumeTarget::Process(name)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(serde_saphyr::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read config file: {err}"),
            Self::Parse(err) => write!(f, "failed to parse config file: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Parse(err) => Some(err),
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_saphyr::Error> for ConfigError {
    fn from(error: serde_saphyr::Error) -> Self {
        Self::Parse(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_config() {
        let yaml = r#"
# process names are case-insensitive
# you can use 'master' to indicate the master channel, or a list of process names to create a group
# you can use 'mic' to control your mic input level (uses the default recording device)
# you can use 'deej.unmapped' to control all apps that aren't bound to any slider
# this ignores master, system, mic and device-targeting sessions
# windows only - you can use 'deej.current' to control the currently active app
# windows only - you can use a device's full name, i.e. "Speakers (Realtek High Definition Audio)", to bind it. this works for both output and input devices
# windows only - you can use 'system' to control the "system sounds" volume
# important: slider indexes start at 0, regardless of which analog pins you're using!
slider_mapping:
  0: master
  1: chrome.exe
  2: spotify.exe
  3:
    - pathofexile_x64.exe
    - rocketleague.exe
  4: discord.exe

# set this to true if you want the controls inverted (i.e. top is 0%, bottom is 100%)
invert_sliders: false

# settings for connecting to the arduino board
com_port: COM4
baud_rate: 9600

# adjust the amount of signal noise reduction depending on your hardware quality
# supported values are "low" (excellent hardware), "default" (regular hardware) or "high" (bad, noisy hardware)
noise_reduction: default
"#;

        let config: ServiceConfig = serde_saphyr::from_str::<RawConfig>(yaml).unwrap().into();

        assert_eq!(
            config.slider_mapping.get(&0).unwrap(),
            &[VolumeTarget::Master]
        );

        assert_eq!(
            config.slider_mapping.get(&1).unwrap(),
            &[VolumeTarget::Process("chrome.exe".to_string())]
        );

        assert_eq!(
            config.slider_mapping.get(&2).unwrap(),
            &[VolumeTarget::Process("spotify.exe".to_string())]
        );

        assert_eq!(
            config.slider_mapping.get(&3).unwrap(),
            &[
                VolumeTarget::Process("pathofexile_x64.exe".to_string()),
                VolumeTarget::Process("rocketleague.exe".to_string()),
            ]
        );

        assert_eq!(
            config.slider_mapping.get(&4).unwrap(),
            &[VolumeTarget::Process("discord.exe".to_string())]
        );

        assert!(!config.invert_sliders);
        assert_eq!(config.com_port, "COM4");
        assert_eq!(config.baud_rate, 9600);
        assert!(matches!(config.noise_reduction, NoiseReduction::Default));
    }
}
