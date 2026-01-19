use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::exit;

#[derive(Deserialize, Serialize)]
pub struct Config {
    #[serde(rename = "TOKEN")]
    pub token: String,
    #[serde(rename = "API_URL")]
    pub api_url: String,
    #[serde(rename = "RUST_LOG")]
    rust_log: String,
    #[serde(rename = "SUPPORTED_PARTITIONS")]
    pub supported_partitions: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: "YOUR_BOT_TOKEN".to_string(),
            api_url: "https://api.telegram.org".to_string(),
            rust_log: "debug".to_string(),
            supported_partitions: vec![
                "boot".to_string(),
                "dtbo".to_string(),
                "init_boot".to_string(),
                "modem".to_string(),
                "modemfirmware".to_string(),
                "recovery".to_string(),
                "system_dlkm".to_string(),
                "vbmeta".to_string(),
                "vbmeta_system".to_string(),
                "vbmeta_vendor".to_string(),
                "vendor_boot".to_string(),
                "vendor_dlkm".to_string(),
            ],
        }
    }
}

pub fn load_config() -> Result<Config> {
    let config_path = Path::new("config.toml");
    if !config_path.exists() {
        println!("Config file not found, please check config.toml");
        let default_config = Config::default();
        let toml_string =
            toml::to_string_pretty(&default_config).expect("Create default config error");
        fs::write(config_path, toml_string)?;
        exit(1)
    }
    let contents = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&contents).expect("Parse config file error");
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", &config.rust_log);
        }
    }
    Ok(config)
}
