use serde::{Deserialize, Serialize};
use std::fmt;
// use std::time::Duration;
use std::net::SocketAddr;
use uuid::Uuid;

/// 代理状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyStatus {
    /// 可用
    Available,
    /// 正在使用
    InUse,
    /// 失败
    Failed,
    /// 未经测试
    Untested,
    /// 未知
    Unknown,
}

impl Default for ProxyStatus {
    fn default() -> Self {
        Self::Untested
    }
}

impl fmt::Display for ProxyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProxyStatus::Available => write!(f, "Available"),
            ProxyStatus::InUse => write!(f, "In Use"),
            ProxyStatus::Failed => write!(f, "Failed"),
            ProxyStatus::Untested => write!(f, "Untested"),
            ProxyStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// 代理信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyInfo {
    /// 代理地址
    pub host: String,
    /// 代理端口
    pub port: u16,
    /// 用户名（可选）
    pub username: Option<String>,
    /// 密码（可选）
    pub password: Option<String>,
    /// 代理类型
    pub proxy_type: String,
    /// 位置/标签信息
    pub location: Option<String>,
    /// 最后测速结果 (毫秒)
    pub last_latency: Option<u64>,
    /// 成功率 (0.0-1.0)
    pub success_rate: f64,
    /// 最后检查时间
    pub last_checked: Option<chrono::DateTime<chrono::Utc>>,
    /// 当前状态
    pub status: ProxyStatus,
}

impl ProxyInfo {
    /// 创建新的代理信息
    pub fn new(host: &str, port: u16, username: Option<String>, password: Option<String>) -> Self {
        Self {
            host: host.to_string(),
            port,
            username,
            password,
            proxy_type: "socks5".to_string(),
            location: None,
            last_latency: None,
            success_rate: 0.0,
            last_checked: None,
            status: ProxyStatus::Untested,
        }
    }

    /// 获取代理地址
    pub fn socket_addr(&self) -> Result<SocketAddr, std::io::Error> {
        format!("{}:{}", self.host, self.port).parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    }
}

/// 代理实现
#[derive(Debug, Clone)]
pub struct Proxy {
    /// 唯一标识符
    pub id: String,
    /// 代理信息
    pub info: ProxyInfo,
    /// 代理状态
    pub status: ProxyStatus,
    /// 延迟（毫秒）
    pub latency: u64,
    /// 最后测试时间
    pub last_tested: Option<chrono::DateTime<chrono::Utc>>,
}

impl Proxy {
    /// 创建新代理
    pub fn new(
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        let info = ProxyInfo {
            host,
            port,
            username,
            password,
            proxy_type: "socks5".to_string(),
            location: None,
            last_latency: None,
            success_rate: 0.0,
            last_checked: None,
            status: ProxyStatus::Untested,
        };

        Self {
            id: Uuid::new_v4().to_string(),
            info,
            status: ProxyStatus::Unknown,
            latency: u64::MAX,
            last_tested: None,
        }
    }

    /// 获取代理URL
    pub fn url(&self) -> String {
        match (&self.info.username, &self.info.password) {
            (Some(user), Some(pass)) => {
                format!("{}://{}:{}@{}:{}", self.info.proxy_type, user, pass, self.info.host, self.info.port)
            }
            _ => format!("{}://{}:{}", self.info.proxy_type, self.info.host, self.info.port),
        }
    }

    /// 更新代理状态
    pub fn update_status(&mut self, status: ProxyStatus) {
        self.status = status;
        self.info.status = status;
    }

    /// 更新代理状态和延迟
    pub fn update_status_and_latency(&mut self, status: ProxyStatus, latency: Option<u64>) {
        self.update_status(status);
        if let Some(lat) = latency {
            self.latency = lat;
            self.update_latency(lat);
        }
        self.last_tested = Some(chrono::Utc::now());
    }

    /// 更新延迟信息
    pub fn update_latency(&mut self, latency_ms: u64) {
        self.info.last_latency = Some(latency_ms);
        self.info.last_checked = Some(chrono::Utc::now());
    }

    /// 更新成功率
    pub fn update_success_rate(&mut self, success: bool) {
        // 简单实现，实际应该考虑历史记录
        let old_rate = self.info.success_rate;
        let weight = 0.7; // 新结果权重
        self.info.success_rate = old_rate * (1.0 - weight) + (if success { 1.0 } else { 0.0 }) * weight;
    }
}
