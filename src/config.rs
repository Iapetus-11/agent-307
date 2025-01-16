use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use home::home_dir;
use serde::{Deserialize, Serialize};

pub static CONFIG_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    let mut path = home_dir().unwrap();
    path.push(".config/agent-307/config.json");
    path.into_boxed_path()
});

static DEFAULT_RECORDINGS_PATH: LazyLock<Box<Path>> = LazyLock::new(|| {
    let mut path = home_dir().unwrap();

    if cfg!(target_os = "macos") {
        path.push("Movies/Agent 307");
    } else if cfg!(target_os = "linux") {
        path.push("Videos/Agent 307");
    } else {
        todo!("Add support for this target OS");
    }

    path.into_boxed_path()
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
            recordings_dir: DEFAULT_RECORDINGS_PATH.to_path_buf(),
        }
    }
}

pub fn load_config() -> Config {
    let config_saved = CONFIG_PATH.to_path_buf().exists();

    let config: Config = if !config_saved {
        Config::default()
    } else {
        let file = File::open(&*CONFIG_PATH).unwrap();
        serde_json::from_reader(file).unwrap()
    };

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
