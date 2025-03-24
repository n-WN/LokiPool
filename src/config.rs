use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub proxy: ProxyConfig,
    pub log: LogConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub max_connections: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    pub proxy_file: String,
    pub test_timeout: u64,
    pub health_check_interval: u64,
    pub retry_times: u32,
    pub auto_switch: bool,
    pub switch_interval: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogConfig {
    pub show_connection_log: bool,
    pub show_error_log: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                bind_host: "127.0.0.1".to_string(),
                bind_port: 1080,
                max_connections: 100,
            },
            proxy: ProxyConfig {
                proxy_file: "proxies.txt".to_string(),
                test_timeout: 5,
                health_check_interval: 300,
                retry_times: 3,
                auto_switch: false,
                switch_interval: 300,
            },
            log: LogConfig {
                show_connection_log: true,
                show_error_log: false,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Path::new("config.toml");
        
        if !config_path.exists() {
            let config = Config::default();
            let toml = toml::to_string_pretty(&config)?;
            fs::write(config_path, toml)?;
            return Ok(config);
        }
        
        let content = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
} 