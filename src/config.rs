use std::{
    cell::LazyCell,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use home::home_dir;
use serde::{Deserialize, Serialize};

pub const CONFIG_PATH: LazyCell<PathBuf> = LazyCell::new(|| {
    let mut path = home_dir().unwrap();
    path.push(".config/agent-307/config.json");
    path
});

const DEFAULT_RECORDINGS_PATH: LazyCell<PathBuf> = LazyCell::new(|| {
    let mut path = home_dir().unwrap();

    if cfg!(target_os = "macos") {
        path.push("Movies/Agent 307");
    } else {
        todo!();
    }

    path
});

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VideoDeviceRecordingConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VideoDeviceConfig {
    pub idx: i32,
    #[serde(default)]
    pub recording: VideoDeviceRecordingConfig,
    #[serde(default)]
    pub max_resolution_width: Option<u16>,
}

fn config_video_device_configs_default() -> Vec<VideoDeviceConfig> {
    vec![VideoDeviceConfig::default()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "config_video_device_configs_default")]
    pub video_devices: Vec<VideoDeviceConfig>,
    pub recordings_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            video_devices: config_video_device_configs_default(),
            recordings_dir: DEFAULT_RECORDINGS_PATH.clone(),
        }
    }
}

pub fn load_config() -> Config {
    let config_saved = CONFIG_PATH.exists();

    let config: Config;
    if !config_saved {
        config = Config::default();
    } else {
        let file = File::open(&*CONFIG_PATH).unwrap();
        config = serde_json::from_reader(file).unwrap();
    }

    if !config_saved {
        fs::create_dir_all(CONFIG_PATH.parent().unwrap()).unwrap();
    }

    // Save always to write in defaults
    let mut file = File::create(&*CONFIG_PATH).unwrap();
    file.write_all(serde_json::to_string_pretty(&config).unwrap().as_bytes())
        .unwrap();
    file.flush().unwrap();

    config
}
