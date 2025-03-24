//! LokiPool Core - SOCKS5 proxy pool manager with latency testing (core functionality)
//! 
//! This library provides the core functionality for managing and testing SOCKS5 proxies.

// 导出模块
pub mod config;
pub mod error;
pub mod pool;
pub mod proxy;
pub mod tester;
pub mod proxy_pool;

// 从模块导出核心类型
pub use config::{Config, ProxyConfig};
pub use error::{Error, Result};
pub use pool::{Pool, PoolManager, PoolOptions};
pub use proxy::{Proxy, ProxyInfo, ProxyStatus};
pub use tester::{Tester, TestOptions, TestResult};
pub use proxy_pool::{ProxyPool, ProxyEntry};

/// Initialize the logger with default settings
pub fn init_logger() {
    use tracing_subscriber::{fmt, EnvFilter};
    
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .init();
}
