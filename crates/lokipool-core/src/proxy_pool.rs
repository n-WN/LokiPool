// 从根目录的src/proxy_pool.rs复制并修改,以对接core库的其他模块
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::path::Path;
use tokio::sync::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use reqwest::Proxy;
use tokio::time::timeout;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::net::TcpStream;
use std::net::SocketAddr;
use crate::config::Config;
use std::error::Error as StdError;
use std::collections::HashSet;
use tracing::info;

#[derive(Clone, Debug)]
pub struct ProxyEntry {
    pub address: String,
    pub latency: Duration,
    pub last_check: Instant,
    pub fail_count: u32,
}

pub struct ProxyPool {
    proxies: Arc<RwLock<Vec<ProxyEntry>>>,
    current_index: Arc<RwLock<usize>>,
    config: Arc<Config>,
    proxy_file: Arc<String>,
}

impl ProxyPool {
    pub fn new(config: Config) -> Self {
        ProxyPool {
            proxies: Arc::new(RwLock::new(Vec::new())),
            current_index: Arc::new(RwLock::new(0)),
            config: Arc::new(config.clone()),
            proxy_file: Arc::new(config.proxy.proxy_file),
        }
    }

    pub fn get_config(&self) -> &Arc<Config> {
        &self.config
    }

    pub async fn load_from_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = File::open(&path)?;
        let reader = io::BufReader::new(file);
        let mut proxies = HashSet::new();

        // 读取并去重代理地址
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                proxies.insert(line.trim().to_string());
            }
        }

        info!("开始测试代理...");
        let pb = ProgressBar::new(proxies.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        // 创建测试任务
        let mut test_futures = Vec::new();
        for proxy in proxies {
            let pb = pb.clone();
            let config = self.config.clone();
            test_futures.push(tokio::spawn(async move {
                let client = reqwest::Client::builder()
                    .proxy(Proxy::all(format!("socks5://{}", proxy))?)
                    .build()?;

                let start = Instant::now();
                match timeout(Duration::from_secs(config.proxy.test_timeout), async {
                    // 先发送HEAD请求检查连接性
                    let resp = client.head("http://www.baidu.com")
                        .send()
                        .await?;
                    
                    if !resp.status().is_success() {
                        return Err(anyhow::anyhow!("HTTP状态码错误: {}", resp.status()));
                    }
                    
                    // 如果HEAD请求成功，再发送GET请求测试实际访问
                    let resp = client.get("http://www.baidu.com")
                        .send()
                        .await?;
                    
                    if !resp.status().is_success() {
                        return Err(anyhow::anyhow!("HTTP状态码错误: {}", resp.status()));
                    }
                    
                    // 确保能读取响应内容
                    let _body = resp.bytes().await?;
                    Ok::<(), anyhow::Error>(())
                }).await {
                    Ok(Ok(_)) => {
                        pb.inc(1);
                        Ok((proxy, start.elapsed()))
                    },
                    Ok(Err(_)) => {
                        pb.inc(1);
                        Err(anyhow::anyhow!("代理无法正常访问目标网站"))
                    },
                    Err(_) => {
                        pb.inc(1);
                        Err(anyhow::anyhow!("代理访问超时"))
                    },
                }
            }));
        }

        // 等待所有测试完成
        let mut valid_proxies = Vec::new();
        let mut invalid_proxies = Vec::new();

        for future in test_futures {
            match future.await {
                Ok(Ok((addr, latency))) => {
                    if latency <= Duration::from_secs(self.config.proxy.test_timeout) {
                        valid_proxies.push(ProxyEntry {
                            address: addr.clone(),
                            latency,
                            last_check: Instant::now(),
                            fail_count: 0,
                        });
                    } else {
                        invalid_proxies.push(addr);
                    }
                }
                Ok(Err(_)) => {
                    // 在错误情况下记录为无效代理
                    invalid_proxies.push("unknown".to_string());
                }
                Err(_) => continue,
            }
        }

        pb.finish_with_message("代理测试完成");

        // 按延迟排序
        valid_proxies.sort_by(|a, b| a.latency.cmp(&b.latency));

        // 更新代理列表
        let mut pool = self.proxies.write().await;
        *pool = valid_proxies.clone(); // 克隆一份用于更新内存中的代理池

        // 更新文件中的代理列表（只保留有效代理）
        let valid_proxies_str: Vec<String> = valid_proxies.iter()
            .map(|p| p.address.clone())
            .collect();
        fs::write(&path, valid_proxies_str.join("\n"))?;

        info!("\n{} {} {}", 
            "测试完成，可用代理:".green().bold(), 
            pool.len().to_string().yellow().bold(),
            "个".green().bold()
        );
        
        if !invalid_proxies.is_empty() {
            info!("{} {} {}", 
                "已删除无效代理:".yellow().bold(),
                invalid_proxies.len().to_string().red().bold(),
                "个".yellow().bold()
            );
        }
        
        // 显示延迟信息
        for (i, proxy) in pool.iter().enumerate() {
            let latency = proxy.latency.as_millis();
            let latency_str = match latency {
                0..=100 => latency.to_string().green(),
                101..=300 => latency.to_string().yellow(),
                _ => latency.to_string().red(),
            };
            info!("{:3}. {} - {}ms", 
                (i + 1).to_string().blue().bold(),
                proxy.address.cyan(),
                latency_str
            );
        }
        info!("健康检查任务已启动");

        // 启动健康检查任务
        self.start_health_check();

        Ok(())
    }

    // 在健康检查中也同步更新文件
    fn start_health_check(&self) {
        let pool = Arc::clone(&self.proxies);
        let config = Arc::clone(&self.config);
        let proxy_file = Arc::clone(&self.proxy_file);
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(config.proxy.health_check_interval)).await;
                
                let mut proxies = pool.write().await;
                let mut i = 0;

                while i < proxies.len() {
                    let addr = proxies[i].address.clone();
                    match Self::test_proxy_health(&addr).await {
                        Ok(latency) => {
                            proxies[i].latency = latency;
                            proxies[i].last_check = Instant::now();
                            proxies[i].fail_count = 0;
                            i += 1;
                        }
                        Err(_) => {
                            proxies[i].fail_count += 1;
                            if proxies[i].fail_count >= config.proxy.retry_times {
                                let removed = proxies.remove(i);
                                info!("{} {}", "代理失效，已移除:".red().bold(), removed.address);
                            } else {
                                i += 1;
                            }
                        }
                    }
                }
                
                // 重新按延迟排序
                proxies.sort_by(|a, b| a.latency.cmp(&b.latency));

                // 更新文件中的代理列表
                if !proxies.is_empty() {
                    let valid_proxies_str: Vec<String> = proxies.iter()
                        .map(|p| p.address.clone())
                        .collect();
                    if let Err(e) = fs::write(&*proxy_file, valid_proxies_str.join("\n")) {
                        eprintln!("{} {}", "更新代理文件失败:".red().bold(), e);
                    }
                }
            }
        });
    }

    async fn test_proxy_health(proxy_addr: &str) -> anyhow::Result<Duration> {
        let client = reqwest::Client::builder()
            .proxy(Proxy::all(format!("socks5://{}", proxy_addr))?)
            .build()?;

        let start = Instant::now();
        let resp = timeout(Duration::from_secs(3), client.head("http://www.baidu.com").send()).await??;
        
        if resp.status().is_success() {
            Ok(start.elapsed())
        } else {
            Err(anyhow::anyhow!("健康检查失败"))
        }
    }

    pub async fn get_connection(&self) -> Result<TcpStream, Box<dyn StdError>> {
        if let Some(proxy) = self.get_current_proxy().await {
            let addr: SocketAddr = proxy.address.parse()?;
            Ok(TcpStream::connect(addr).await?)
        } else {
            Err("没有可用的代理".into())
        }
    }

    pub async fn get_current_proxy(&self) -> Option<ProxyEntry> {
        let proxies = self.proxies.read().await;
        let index = *self.current_index.read().await;
        proxies.get(index).cloned()
    }

    pub async fn next_proxy(&self) -> Option<ProxyEntry> {
        let mut index = self.current_index.write().await;
        let proxies = self.proxies.read().await;
        
        if proxies.is_empty() {
            return None;
        }

        *index = (*index + 1) % proxies.len();
        proxies.get(*index).cloned()
    }

    pub async fn list_proxies(&self) -> Vec<ProxyEntry> {
        self.proxies.read().await.clone()
    }
}
