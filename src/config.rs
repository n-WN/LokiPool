use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

/// 主配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 全局超时设置（毫秒）
    pub timeout_ms: u64,
    /// 最大并发连接数
    pub max_connections: usize,
    /// 重试次数
    pub retry_count: usize,
    /// 代理列表
    pub proxies: Vec<ProxyConfig>,
    /// 测试URL
    pub test_urls: Vec<String>,
}

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

impl Config {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// 创建默认配置
    pub fn default() -> Self {
        Self {
            timeout_ms: 5000,
            max_connections: 100,
            retry_count: 3,
            proxies: vec![],
            test_urls: vec![
                "https://www.google.com".to_string(),
                "https://www.github.com".to_string(),
            ],
        }
    }
}