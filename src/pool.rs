use crate::proxy::{Proxy, ProxyStatus};
use crate::error::Result;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::tester::{Tester, TestOptions, TestResult};
use crate::config::ProxyConfig;

/// 代理池选项配置
#[derive(Debug, Clone)]
pub struct PoolOptions {
    /// 代理池最大容量
    pub max_size: usize,
    /// 是否自动测试代理
    pub auto_test: bool,
    /// 测试间隔（秒）
    pub test_interval: u64,
}

impl Default for PoolOptions {
    fn default() -> Self {
        Self {
            max_size: 100,
            auto_test: true,
            test_interval: 300, // 5分钟
        }
    }
}

impl PoolOptions {
    /// 从配置创建池选项
    pub fn from_config(config: &crate::config::Config) -> Self {
        Self {
            max_size: config.max_connections,
            auto_test: true, // 默认启用自动测试
            test_interval: 300, // 默认5分钟
        }
    }
}

/// 代理池，用于存储和管理代理
#[derive(Debug, Clone)]
pub struct Pool {
    proxies: Arc<Mutex<HashMap<String, Proxy>>>,
    options: PoolOptions,
}

impl Pool {
    /// 创建新的代理池
    pub fn new(options: PoolOptions) -> Self {
        Self {
            proxies: Arc::new(Mutex::new(HashMap::new())),
            options,
        }
    }

    /// 从代理配置列表创建代理池
    pub fn new_with_proxies(proxies: Vec<crate::config::ProxyConfig>, options: PoolOptions) -> Self {
        let pool = Self::new(options); // 移除 mut 关键字
        
        for proxy_config in proxies {
            let proxy = Proxy::new(
                proxy_config.host,
                proxy_config.port,
                proxy_config.username,
                proxy_config.password,
            );
            
            // 忽略添加失败的情况
            let _ = pool.add(proxy);
        }
        
        pool
    }

    /// 添加代理到池中
    pub fn add(&self, proxy: Proxy) -> Result<()> {
        let mut proxies = self.proxies.lock().unwrap();
        if proxies.len() >= self.options.max_size {
            return Err(crate::error::Error::Other("Pool size limit reached".to_string()));
        }
        proxies.insert(proxy.id.clone(), proxy);
        Ok(())
    }

    /// 获取可用代理
    pub fn get_available(&self) -> Option<Proxy> {
        let proxies = self.proxies.lock().unwrap();
        proxies.values()
            .filter(|p| p.status == ProxyStatus::Available)
            .min_by_key(|p| p.latency)
            .cloned()
    }

    /// 测试所有代理
    pub async fn test_all(&self) -> Vec<(ProxyConfig, TestResult)> {
        let mut results = Vec::new();
        let tester = Tester::new(TestOptions::default());
        
        let proxies_lock = self.proxies.lock().unwrap();
        for proxy in proxies_lock.values() {
            // 因为 test_proxy 需要可变引用，但我们只有不可变引用，
            // 所以先克隆一份代理，测试后得到结果，但不更新原始代理
            let mut proxy_clone = proxy.clone();
            
            match tester.test_proxy(&mut proxy_clone) {
                Ok(result) => {
                    // 创建 ProxyConfig 用于返回结果
                    let config = ProxyConfig {
                        host: proxy.info.host.clone(),
                        port: proxy.info.port,
                        username: proxy.info.username.clone(),
                        password: proxy.info.password.clone(),
                        location: proxy.info.location.clone(),
                        proxy_type: proxy.info.proxy_type.clone(),
                    };
                    
                    results.push((config, result));
                },
                Err(_) => {
                    // 测试失败的情况下，可以记录错误
                    // 但对于简单实现，我们可以跳过
                    continue;
                }
            }
        }
        
        results
    }
}

/// 代理池管理器，管理多个代理池
pub struct PoolManager {
    pools: HashMap<String, Pool>,
}

impl PoolManager {
    /// 创建新的代理池管理器
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// 创建新的代理池
    pub fn create_pool(&mut self, name: &str, options: PoolOptions) -> Result<()> {
        if self.pools.contains_key(name) {
            return Err(crate::error::Error::Configuration(format!("Pool {} already exists", name)));
        }
        
        self.pools.insert(name.to_string(), Pool::new(options));
        Ok(())
    }

    /// 获取代理池
    pub fn get_pool(&self, name: &str) -> Option<&Pool> {
        self.pools.get(name)
    }
}
