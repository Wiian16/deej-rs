use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    slider_mapping: HashMap<u8, SliderMapping>,
    invert_sliders: bool,
    com_port: String,
    baud_rate: u32,
    noise_reduction: NoiseReduction,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum SliderMapping {
    Process(String),
    Processes(Vec<String>),
}

impl SliderMapping {
    pub fn processes(&self) -> &[String] {
        match self {
            Self::Process(process) => std::slice::from_ref(process),
            Self::Processes(processes) => processes,
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum NoiseReduction {
    Low,
    Default,
    High,
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

        let config: Config = serde_saphyr::from_str(yaml).unwrap();

        assert_eq!(
            config.slider_mapping.get(&0),
            Some(&SliderMapping::Process("master".to_string()))
        );

        assert_eq!(
            config.slider_mapping.get(&1),
            Some(&SliderMapping::Process("chrome.exe".to_string()))
        );

        assert_eq!(
            config.slider_mapping.get(&2),
            Some(&SliderMapping::Process("spotify.exe".to_string()))
        );

        assert_eq!(
            config.slider_mapping.get(&3),
            Some(&SliderMapping::Processes(vec![
                "pathofexile_x64.exe".to_string(),
                "rocketleague.exe".to_string(),
            ]))
        );

        assert_eq!(
            config.slider_mapping.get(&4),
            Some(&SliderMapping::Process("discord.exe".to_string()))
        );

        assert!(!config.invert_sliders);
        assert_eq!(config.com_port, "COM4");
        assert_eq!(config.baud_rate, 9600);
        assert!(matches!(config.noise_reduction, NoiseReduction::Default));
    }
}
