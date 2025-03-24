//! LokiPool - A SOCKS5 proxy pool manager with latency testing
//! 
//! This library provides functionality for managing and testing SOCKS5 proxies.

// 重导出core库
pub use lokipool_core::{
    Config, ProxyConfig,
    Error, Result,
    Pool, PoolManager, PoolOptions,
    Proxy, ProxyInfo, ProxyStatus,
    Tester, TestOptions, TestResult,
    ProxyPool, ProxyEntry,
    init_logger
};

// 本地模块
pub mod socks_server;
// 移除这行，因为我们不再需要自己的proxy_pool实现
// mod proxy_pool;

// 可选的命令行界面
#[cfg(feature = "ui")]
pub mod ui;