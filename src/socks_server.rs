use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::{Result, anyhow};
use std::sync::Arc;
// 修改导入路径，使用lokipool_core而不是lokipool
use lokipool_core::Pool;
use tracing::{info, error, warn, debug}; // 引入debug日志级别
use tokio::sync::broadcast;
// use std::error::Error as StdError; // 导入StdError
use std::net::{Ipv4Addr, Ipv6Addr}; // 导入Ipv6Addr

/// SOCKS5服务器配置
#[derive(Debug, Clone)]
pub struct SocksServerConfig {
    /// 监听地址
    pub bind_address: String,
    /// 监听端口
    pub bind_port: u16,
}

impl Default for SocksServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            bind_port: 1080,
        }
    }
}

/// SOCKS5 代理服务器
pub struct SocksServer {
    config: SocksServerConfig,
    pool: Arc<Pool>,
}

impl SocksServer {
    /// 创建新的SOCKS5服务器
    pub fn new(socks_config: SocksServerConfig, pool: Pool) -> Self {
        Self {
            config: socks_config,
            pool: Arc::new(pool),
        }
    }

    #[allow(dead_code)]
    /// 启动SOCKS5服务器
    pub async fn run(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.bind_port);
        let listener = TcpListener::bind(&addr).await?;
        
        info!("SOCKS5服务器开始监听: {}", addr);
        
        loop {
            match listener.accept().await {
                Ok((stream, client_addr)) => {
                    let pool = Arc::clone(&self.pool);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, client_addr, pool).await {
                            error!("处理连接出错: {}", e);
                        }
                    });
                }
                Err(e) => {
                    warn!("接受连接失败: {}", e);
                }
            }
        }
    }

    /// 启动SOCKS5服务器，可以通过接收shutdown信号优雅关闭
    pub async fn run_with_shutdown(&self, mut shutdown: broadcast::Receiver<()>) -> Result<()> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.bind_port);
        let listener = TcpListener::bind(&addr).await?;
        
        info!("SOCKS5服务器开始监听: {}", addr);
        
        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, client_addr)) => {
                            let pool = Arc::clone(&self.pool);
                            let mut shutdown_clone = shutdown.resubscribe();
                            tokio::spawn(async move {
                                tokio::select! {
                                    conn_result = Self::handle_connection(stream, client_addr, pool) => {
                                        if let Err(e) = conn_result {
                                            error!("处理连接出错: {}", e);
                                        }
                                    },
                                    _ = shutdown_clone.recv() => {
                                        info!("连接处理器收到关闭信号");
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            warn!("接受连接失败: {}", e);
                        }
                    }
                },
                _ = shutdown.recv() => {
                    info!("SOCKS5服务器收到关闭信号，正在停止...");
                    break;
                }
            }
        }
        
        Ok(())
    }

    /// 处理SOCKS5连接
    async fn handle_connection(
        stream: TcpStream, 
        client_addr: SocketAddr,
        pool: Arc<Pool>
    ) -> Result<()> {
        info!("接受来自 {} 的新连接", client_addr);
        
        // 改进错误处理，添加更多诊断信息
        let handle_err = |step: &str, e: anyhow::Error| -> Result<()> {
            error!("SOCKS5 {}失败: {} (来自: {})", step, e, client_addr);
            Err(anyhow!("{}: {}", step, e))
        };
        
        // 1. 认证方法协商
        let (mut inbound_reader, mut inbound_writer) = stream.into_split();
        
        // 读取客户端支持的认证方法
        let mut method_selection = [0u8; 2];
        match inbound_reader.read_exact(&mut method_selection).await {
            Ok(_) => {
                debug!("收到认证方法协商请求: {:x?}", method_selection);
                if method_selection[0] != 0x05 { // SOCKS5
                    let e = anyhow!("收到非SOCKS5请求: 版本={}", method_selection[0]);
                    return handle_err("协议版本检查", e);
                }
            }
            Err(e) => {
                warn!("来自 {} 的连接在认证方法读取时断开: {}", client_addr, e);
                return Ok(()); // 直接返回，不认为是严重错误
            }
        }
        
        let nmethods = method_selection[1] as usize;
        let mut methods = vec![0u8; nmethods];
        inbound_reader.read_exact(&mut methods).await?;
        debug!("客户端支持的认证方法: {:x?}", methods);

        // 回复使用无认证方法
        debug!("回复客户端使用无认证方法");
        inbound_writer.write_all(&[0x05, 0x00]).await?;
        inbound_writer.flush().await?;
        
        // 2. 读取请求
        let mut buf = [0u8; 4];
        match inbound_reader.read_exact(&mut buf).await {
            Ok(_) => {
                debug!("收到连接请求: {:x?}", buf);
                if buf[0] != 0x05 || buf[1] != 0x01 {
                    let e = anyhow!("不支持的SOCKS5命令: VER={}, CMD={}", buf[0], buf[1]);
                    return handle_err("命令检查", e);
                }
            }
            Err(e) => {
                let e = anyhow!("读取命令时发生错误: {}", e);
                return handle_err("读取命令", e);
            }
        }
        
        // 3. 读取目标地址
        let atyp = buf[3];
        let target_addr = match atyp {
            0x01 => { // IPv4
                let mut addr = [0u8; 4];
                inbound_reader.read_exact(&mut addr).await?;
                let ipv4 = Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
                let addr_str = ipv4.to_string();
                debug!("目标地址类型: IPv4, 地址: {}", addr_str);
                addr_str
            },
            0x03 => { // 域名
                let len = inbound_reader.read_u8().await? as usize;
                let mut domain = vec![0u8; len];
                inbound_reader.read_exact(&mut domain).await?;
                let domain_str = String::from_utf8(domain)?;
                debug!("目标地址类型: 域名, 地址: {}", domain_str);
                domain_str
            },
            0x04 => { // IPv6
                let mut addr = [0u8; 16];
                inbound_reader.read_exact(&mut addr).await?;
                let ipv6 = Ipv6Addr::new(
                    ((addr[0] as u16) << 8) | (addr[1] as u16),
                    ((addr[2] as u16) << 8) | (addr[3] as u16),
                    ((addr[4] as u16) << 8) | (addr[5] as u16),
                    ((addr[6] as u16) << 8) | (addr[7] as u16),
                    ((addr[8] as u16) << 8) | (addr[9] as u16),
                    ((addr[10] as u16) << 8) | (addr[11] as u16),
                    ((addr[12] as u16) << 8) | (addr[13] as u16),
                    ((addr[14] as u16) << 8) | (addr[15] as u16),
                );
                let addr_str = ipv6.to_string();
                debug!("目标地址类型: IPv6, 地址: {}", addr_str);
                addr_str
            },
            _ => return Err(anyhow::anyhow!("不支持的地址类型")),
        };
        
        // 4. 读取端口
        let port = inbound_reader.read_u16().await?;
        debug!("目标端口: {}", port);
        
        // 5. 获取代理
        let proxy = match pool.get_available() {
            Some(p) => {
                info!("找到可用代理: {}:{}", p.info.host, p.info.port);
                p
            },
            None => {
                // 添加更多日志以便调试
                let proxies = pool.get_all_proxies();
                error!("没有可用的代理，当前有 {} 个代理", proxies.len());
                
                for proxy in proxies {
                    error!("代理 {}:{} 状态: {:?}, 延迟: {}ms", 
                            proxy.info.host, proxy.info.port, 
                            proxy.status, proxy.latency);
                }
                
                return Err(anyhow::anyhow!("没有可用的代理"));
            }
        };
        
        info!("使用代理 {}:{} 连接到 {}:{}", proxy.info.host, proxy.info.port, target_addr, port);
        
        // 6. 连接到目标地址（通过代理）
        let proxy_addr = proxy.info.socket_addr()?;
        debug!("连接到上游代理: {}", proxy_addr);
        let mut upstream = TcpStream::connect(proxy_addr).await?;
        
        // 7. 与上游SOCKS5服务器进行握手
        info!("向上游代理 {}:{} 发送握手请求", proxy.info.host, proxy.info.port);
        upstream.write_all(&[0x05, 0x01, 0x00]).await?;
        let mut response = [0u8; 2];
        match upstream.read_exact(&mut response).await {
            Ok(_) => {
                debug!("收到上游代理握手响应: {:x?}", response);
                if response[0] != 0x05 || response[1] != 0x00 {
                    let e = anyhow!("上游代理握手失败: VER={}, METHOD={}", response[0], response[1]);
                    return handle_err("上游代理握手", e);
                }
                info!("上游代理握手成功");
            }
            Err(e) => {
                let e = anyhow!("读取上游代理握手响应失败: {}", e);
                return handle_err("读取上游代理握手响应", e);
            }
        }
        
        // 8. 发送连接请求到上游代理
        let mut request = Vec::new();
        request.extend_from_slice(&[0x05, 0x01, 0x00]); // VER, CMD, RSV
        
        match atyp {
            0x01 => { // IPv4
                request.push(0x01);
                for octet in target_addr.split('.') {
                    request.push(octet.parse::<u8>()?);
                }
            },
            0x03 => { // Domain
                request.push(0x03);
                request.push(target_addr.len() as u8);
                request.extend_from_slice(target_addr.as_bytes());
            },
            0x04 => { // IPv6
                request.push(0x04);
                let ipv6 = target_addr.parse::<Ipv6Addr>()?;
                for segment in ipv6.segments() {
                    request.extend_from_slice(&segment.to_be_bytes());
                }
            },
            _ => return Err(anyhow::anyhow!("不支持的地址类型")),
        }
        
        // 添加端口
        request.extend_from_slice(&port.to_be_bytes());
        
        debug!("向上游代理发送连接请求: 目标={}:{}, 请求内容: {:x?}", target_addr, port, request);
        info!("向上游代理发送连接请求: 目标={}:{}", target_addr, port);
        upstream.write_all(&request).await?;
        
        // 9. 读取上游代理响应
        let mut response = [0u8; 4];
        match upstream.read_exact(&mut response).await {
            Ok(_) => {
                debug!("收到上游代理连接目标响应: {:x?}", response);
                if response[1] != 0x00 {
                    let e = anyhow!("上游代理连接目标失败: {}", response[1]);
                    return handle_err("上游代理连接目标", e);
                }
                info!("上游代理连接目标成功");
            }
            Err(e) => {
                let e = anyhow!("读取上游代理连接目标响应失败: {}", e);
                return handle_err("读取上游代理连接目标响应", e);
            }
        }
        
        // 10. 跳过绑定地址和端口
        match response[3] {
            0x01 => { // IPv4
                let mut addr = [0u8; 4];
                upstream.read_exact(&mut addr).await?;
                let ipv4 = Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
                debug!("上游代理返回的绑定地址: IPv4={:?}", ipv4);
            },
            0x03 => { // Domain
                let len = upstream.read_u8().await?;
                let mut domain = vec![0u8; len as usize];
                upstream.read_exact(&mut domain).await?;
                debug!("上游代理返回的绑定地址: Domain={:?}", String::from_utf8(domain.clone()));
            },
            0x04 => { // IPv6
                let mut addr = [0u8; 16];
                upstream.read_exact(&mut addr).await?;
                let ipv6 = Ipv6Addr::new(
                    ((addr[0] as u16) << 8) | (addr[1] as u16),
                    ((addr[2] as u16) << 8) | (addr[3] as u16),
                    ((addr[4] as u16) << 8) | (addr[5] as u16),
                    ((addr[6] as u16) << 8) | (addr[7] as u16),
                    ((addr[8] as u16) << 8) | (addr[9] as u16),
                    ((addr[10] as u16) << 8) | (addr[11] as u16),
                    ((addr[12] as u16) << 8) | (addr[13] as u16),
                    ((addr[14] as u16) << 8) | (addr[15] as u16),
                );
                debug!("上游代理返回的绑定地址: IPv6={:?}", ipv6);
            },
            _ => return Err(anyhow::anyhow!("上游代理返回了不支持的地址类型")),
        }
        let mut port = [0u8; 2];
        upstream.read_exact(&mut port).await?;
        debug!("上游代理返回的绑定端口: {:?}", port);
        
        // 11. 发送成功响应给客户端
        let response = [
            0x05, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        debug!("向客户端发送连接成功响应: {:x?}", response);
        inbound_writer.write_all(&response).await?;
        
        // 12. 双向转发数据
        let (mut upstream_reader, mut upstream_writer) = upstream.into_split();
        let client_to_proxy = tokio::io::copy(&mut inbound_reader, &mut upstream_writer);
        let proxy_to_client = tokio::io::copy(&mut upstream_reader, &mut inbound_writer);
        
        info!("开始双向转发数据");
        tokio::select! {
            res = client_to_proxy => {
                match res {
                    Ok(bytes) => debug!("客户端 -> 代理 传输完成, {} bytes", bytes),
                    Err(e) => error!("客户端到代理传输错误: {}", e),
                }
            },
            res = proxy_to_client => {
                match res {
                    Ok(bytes) => debug!("代理 -> 客户端 传输完成, {} bytes", bytes),
                    Err(e) => error!("代理到客户端传输错误: {}", e),
                }
            }
        }
        
        Ok(())
    }
}