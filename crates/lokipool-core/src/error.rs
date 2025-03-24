use std::io;

/// Error type for LokiPool operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// IO操作错误
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    /// 代理连接错误
    #[error("Connection error: {0}")]
    Connection(String),
    /// 验证错误
    #[error("Authentication error: {0}")]
    Authentication(String),
    /// 配置错误
    #[error("Configuration error: {0}")]
    Configuration(String),
    /// 测试错误
    #[error("Test error: {0}")]
    Test(String),
    /// 其他错误
    #[error("Error: {0}")]
    Other(String),
    /// 超时错误
    #[error("Connection timed out after {0}ms")]
    Timeout(u64),
    /// 代理连接错误
    #[error("Proxy connection failed: {0}")]
    ProxyConnection(String),
    /// 请求错误
    #[error("Request failed: {0}")]
    Request(String),
    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),
}

// 移除手动实现的 Display 和 std::error::Error trait
// 因为 thiserror::Error 已经提供了这些实现

/// Result type for LokiPool operations
pub type Result<T> = std::result::Result<T, Error>;

/// 从reqwest错误转换
impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Error::Timeout(5000) // 默认超时值
        } else if err.is_connect() {
            Error::ProxyConnection(err.to_string())
        } else {
            Error::Request(err.to_string())
        }
    }
}

/// 从toml错误转换
impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

/// 从toml序列化错误转换
impl From<toml::ser::Error> for Error {
    fn from(err: toml::ser::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}
