use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use crate::error::Result;
use tracing::{info, warn};

/// 主配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 全局超时设置（毫秒）
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// 最大并发连接数
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    /// 重试次数
    #[serde(default = "default_retry_count")]
    pub retry_count: usize,
    /// 代理配置
    #[serde(default)]
    pub proxy: ProxySettings,
    /// 代理列表
    #[serde(default)]
    pub proxies: Vec<ProxyConfig>,
    /// 测试URL
    #[serde(default = "default_test_urls")]
    pub test_urls: Vec<String>,
}

fn default_timeout_ms() -> u64 { 10000 }
fn default_max_connections() -> usize { 100 }
fn default_retry_count() -> usize { 3 }
fn default_test_urls() -> Vec<String> { 
    vec!["http://www.baidu.com".to_string()] 
}

/// 代理设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySettings {
    /// 代理文件路径
    #[serde(default = "default_proxy_file")]
    pub proxy_file: String,
    /// 测试超时时间（秒）
    #[serde(default = "default_test_timeout")]
    pub test_timeout: u64,
    /// 健康检查间隔（秒）
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: u64,
    /// 最大重试次数
    #[serde(default = "default_retry_times")]
    pub retry_times: u32,
}

fn default_proxy_file() -> String { "proxies.txt".to_string() }
fn default_test_timeout() -> u64 { 10 }
fn default_health_check_interval() -> u64 { 300 }
fn default_retry_times() -> u32 { 3 }

/// 单个代理的配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// 代理服务器地址
    pub host: String,
    /// 代理服务器端口
    pub port: u16,
    /// 用户名（可选）
    pub username: Option<String>,
    /// 密码（可选）
    pub password: Option<String>,
    /// 代理位置/标签（可选）
    pub location: Option<String>,
    /// 代理类型
    #[serde(default = "default_proxy_type")]
    pub proxy_type: String,
}

fn default_proxy_type() -> String {
    "socks5".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timeout_ms: 10000,
            max_connections: 100,
            retry_count: 3,
            proxy: ProxySettings::default(),
            proxies: Vec::new(),
            test_urls: vec!["http://www.baidu.com".to_string()],
        }
    }
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            proxy_file: "proxies.txt".to_string(),
            test_timeout: 10,
            health_check_interval: 300,
            retry_times: 3,
        }
    }
}

impl Config {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                warn!("无法读取配置文件: {}", e);
                return Err(crate::error::Error::Configuration(
                    format!("无法读取配置文件: {}", e)
                ));
            }
        };
        
        match toml::from_str::<Self>(&content) {
            Ok(config) => {
                info!("成功读取配置: {} 个代理", config.proxies.len());
                Ok(config)
            },
            Err(e) => {
                warn!("配置文件格式错误: {}", e);
                // 尝试使用更宽松的解析方式
                warn!("尝试使用更宽松的解析方式...");
                let config = Self::parse_with_fallbacks(&content)?;
                info!("使用宽松解析成功读取配置: {} 个代理", config.proxies.len());
                Ok(config)
            }
        }
    }

    /// 使用更宽松的解析方式，处理部分字段缺失的情况
    fn parse_with_fallbacks(content: &str) -> Result<Self> {
        // 尝试解析，如果失败则返回默认配置
        let mut config = Config::default();
        
        // 尝试逐个解析各个部分
        if let Ok(parsed_toml) = content.parse::<toml::Table>() {
            // 解析基本字段
            if let Some(timeout) = parsed_toml.get("timeout_ms").and_then(|v| v.as_integer()) {
                config.timeout_ms = timeout as u64;
            }
            
            if let Some(max_conn) = parsed_toml.get("max_connections").and_then(|v| v.as_integer()) {
                config.max_connections = max_conn as usize;
            }
            
            if let Some(retry) = parsed_toml.get("retry_count").and_then(|v| v.as_integer()) {
                config.retry_count = retry as usize;
            }
            
            // 解析测试URL
            if let Some(urls) = parsed_toml.get("test_urls").and_then(|v| v.as_array()) {
                let mut test_urls = Vec::new();
                for url in urls {
                    if let Some(url_str) = url.as_str() {
                        test_urls.push(url_str.to_string());
                    }
                }
                if !test_urls.is_empty() {
                    config.test_urls = test_urls;
                }
            }
            
            // 解析代理设置
            if let Some(proxy_settings) = parsed_toml.get("proxy").and_then(|v| v.as_table()) {
                if let Some(file) = proxy_settings.get("proxy_file").and_then(|v| v.as_str()) {
                    config.proxy.proxy_file = file.to_string();
                }
                
                if let Some(timeout) = proxy_settings.get("test_timeout").and_then(|v| v.as_integer()) {
                    config.proxy.test_timeout = timeout as u64;
                }
                
                if let Some(interval) = proxy_settings.get("health_check_interval").and_then(|v| v.as_integer()) {
                    config.proxy.health_check_interval = interval as u64;
                }
                
                if let Some(retries) = proxy_settings.get("retry_times").and_then(|v| v.as_integer()) {
                    config.proxy.retry_times = retries as u32;
                }
            }
            
            // 解析代理列表
            if let Some(proxies_array) = parsed_toml.get("proxies").and_then(|v| v.as_array()) {
                for proxy_value in proxies_array {
                    if let Some(proxy_table) = proxy_value.as_table() {
                        let host = proxy_table.get("host").and_then(|v| v.as_str())
                            .unwrap_or("127.0.0.1").to_string();
                        
                        let port = proxy_table.get("port").and_then(|v| v.as_integer())
                            .unwrap_or(1080) as u16;
                        
                        let username = proxy_table.get("username").and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        
                        let password = proxy_table.get("password").and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        
                        let location = proxy_table.get("location").and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        
                        let proxy_type = proxy_table.get("proxy_type").and_then(|v| v.as_str())
                            .unwrap_or("socks5").to_string();
                        
                        config.proxies.push(ProxyConfig {
                            host,
                            port,
                            username,
                            password,
                            location,
                            proxy_type,
                        });
                    }
                }
            }
        }
        
        // 如果没有解析到任何代理，添加一个本地默认代理
        if config.proxies.is_empty() {
            config.proxies.push(ProxyConfig {
                host: "127.0.0.1".to_string(),
                port: 1080,
                username: None,
                password: None,
                location: Some("Local Default".to_string()),
                proxy_type: "socks5".to_string(),
            });
            warn!("配置中没有代理，已添加默认本地代理 127.0.0.1:1080");
        }
        
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
