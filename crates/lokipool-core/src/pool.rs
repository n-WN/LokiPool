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
        let pool = Self::new(options);
        
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

    /// 获取所有代理，用于调试
    pub fn get_all_proxies(&self) -> Vec<Proxy> {
        let proxies = self.proxies.lock().unwrap();
        proxies.values().cloned().collect()
    }

    /// 测试所有代理
    pub async fn test_all(&self) -> Vec<(ProxyConfig, TestResult)> {
        let mut results = Vec::new();
        let tester = Tester::new(TestOptions::default());
        
        // 获取锁并修改代理状态
        let mut proxies_lock = self.proxies.lock().unwrap();
        
        for (_, proxy) in proxies_lock.iter_mut() {
            // 克隆代理用于测试
            let mut proxy_clone = proxy.clone();
            
            match tester.test_proxy(&mut proxy_clone) {
                Ok(result) => {
                    // 将测试结果应用回原始代理
                    if result.success {
                        proxy.update_status_and_latency(ProxyStatus::Available, result.latency);
                    } else {
                        proxy.update_status_and_latency(ProxyStatus::Failed, None);
                    }
                    
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
                Err(e) => {
                    // 更新代理状态为失败
                    proxy.update_status(ProxyStatus::Failed);
                    
                    // 创建失败的测试结果
                    let result = TestResult {
                        proxy_id: proxy.id.clone(),
                        success: false,
                        latency: None,
                        error: Some(e.to_string()),
                        timestamp: chrono::Utc::now(),
                    };
                    
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
                }
            }
        }
        
        results
    }

    // 添加自动重试功能，遇到失败连接时
    pub async fn retry_connections(&self) -> bool {
        let mut any_updated = false;
        let mut proxies_lock = self.proxies.lock().unwrap();
        
        // 检查是否有失败的代理需要重试
        let mut failed_proxies: Vec<String> = Vec::new();
        for (id, proxy) in proxies_lock.iter() {
            if proxy.status == ProxyStatus::Failed {
                failed_proxies.push(id.clone());
            }
        }
        
        // 如果有失败的代理，则尝试重新测试
        if !failed_proxies.is_empty() {
            let tester = Tester::new(TestOptions::default());
            
            for id in failed_proxies {
                if let Some(proxy) = proxies_lock.get_mut(&id) {
                    let mut proxy_clone = proxy.clone();
                    if let Ok(result) = tester.test_proxy(&mut proxy_clone) {
                        if result.success {
                            proxy.update_status_and_latency(ProxyStatus::Available, result.latency);
                            any_updated = true;
                        }
                    }
                }
            }
        }
        
        any_updated
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
