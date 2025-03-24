use std::io;
use std::fmt;
use std::net::AddrParseError;

/// Error type for LokiPool operations
#[derive(Debug)]
pub enum Error {
    /// IO操作错误
    Io(io::Error),
    /// 代理连接错误
    Connection(String),
    /// 验证错误
    Authentication(String),
    /// 配置错误
    Configuration(String),
    /// 测试错误
    Test(String),
    /// 其他错误
    Other(String),
    /// 超时错误
    Timeout(u64),
    /// 代理连接错误
    ProxyConnection(String),
    /// 请求错误
    Request(String),
    /// 序列化错误
    Serialization(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::Connection(msg) => write!(f, "Connection error: {}", msg),
            Error::Authentication(msg) => write!(f, "Authentication error: {}", msg),
            Error::Configuration(msg) => write!(f, "Configuration error: {}", msg),
            Error::Test(msg) => write!(f, "Test error: {}", msg),
            Error::Other(msg) => write!(f, "Error: {}", msg),
            Error::Timeout(ms) => write!(f, "Connection timed out after {}ms", ms),
            Error::ProxyConnection(msg) => write!(f, "Proxy connection failed: {}", msg),
            Error::Request(msg) => write!(f, "Request failed: {}", msg),
            Error::Serialization(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

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

// 确保从AddrParseError转换到Error的实现
impl From<AddrParseError> for Error {
    fn from(err: AddrParseError) -> Self {
        Error::Configuration(format!("Invalid address format: {}", err))
    }
}
