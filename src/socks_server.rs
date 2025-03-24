use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use std::sync::Arc;
// 修改导入路径，使用lokipool_core而不是lokipool
use lokipool_core::Pool;
use tracing::{info, error, warn};
use tokio::sync::broadcast;
// use tokio::time::{};

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
        
        // 1. 认证方法协商
        let (mut inbound_reader, mut inbound_writer) = stream.into_split();
        
        // 读取客户端支持的认证方法
        let mut method_selection = [0u8; 2];
        inbound_reader.read_exact(&mut method_selection).await?;
        
        if method_selection[0] != 0x05 { // SOCKS5
            return Err(anyhow::anyhow!("不支持的SOCKS版本"));
        }
        
        let nmethods = method_selection[1] as usize;
        let mut methods = vec![0u8; nmethods];
        inbound_reader.read_exact(&mut methods).await?;

        // 回复使用无认证方法
        inbound_writer.write_all(&[0x05, 0x00]).await?;
        inbound_writer.flush().await?;
        
        // 2. 读取请求
        let mut buf = [0u8; 4];
        inbound_reader.read_exact(&mut buf).await?;
        
        if buf[0] != 0x05 || buf[1] != 0x01 {
            return Err(anyhow::anyhow!("不支持的SOCKS5命令"));
        }
        
        // 3. 读取目标地址
        let atyp = buf[3];
        let target_addr = match atyp {
            0x01 => { // IPv4
                let mut addr = [0u8; 4];
                inbound_reader.read_exact(&mut addr).await?;
                format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3])
            },
            0x03 => { // 域名
                let len = inbound_reader.read_u8().await? as usize;
                let mut domain = vec![0u8; len];
                inbound_reader.read_exact(&mut domain).await?;
                String::from_utf8(domain)?
            },
            0x04 => { // IPv6
                let mut addr = [0u8; 16];
                inbound_reader.read_exact(&mut addr).await?;
                return Err(anyhow::anyhow!("暂不支持IPv6"));
            },
            _ => return Err(anyhow::anyhow!("不支持的地址类型")),
        };
        
        // 4. 读取端口
        let port = inbound_reader.read_u16().await?;
        
        // 5. 获取代理
        let proxy = match pool.get_available() {
            Some(p) => p,
            None => return Err(anyhow::anyhow!("没有可用的代理")),
        };
        
        info!("使用代理 {}:{} 连接到 {}:{}", proxy.info.host, proxy.info.port, target_addr, port);
        
        // 6. 连接到目标地址（通过代理）
        let proxy_addr = proxy.info.socket_addr()?;
        let mut upstream = TcpStream::connect(proxy_addr).await?;
        
        // 7. 与上游SOCKS5服务器进行握手
        upstream.write_all(&[0x05, 0x01, 0x00]).await?;
        let mut response = [0u8; 2];
        upstream.read_exact(&mut response).await?;
        
        if response[0] != 0x05 || response[1] != 0x00 {
            return Err(anyhow::anyhow!("上游代理握手失败"));
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
            _ => unreachable!(),
        }
        
        // 添加端口
        request.extend_from_slice(&port.to_be_bytes());
        
        // 发送请求到上游代理
        upstream.write_all(&request).await?;
        
        // 9. 读取上游代理响应
        let mut response = [0u8; 4];
        upstream.read_exact(&mut response).await?;
        
        if response[1] != 0x00 {
            return Err(anyhow::anyhow!("上游代理连接目标失败: {}", response[1]));
        }
        
        // 10. 跳过绑定地址和端口
        match response[3] {
            0x01 => { // IPv4
                let mut addr = [0u8; 4];
                upstream.read_exact(&mut addr).await?;
            },
            0x03 => { // Domain
                let len = upstream.read_u8().await?;
                let mut domain = vec![0u8; len as usize];
                upstream.read_exact(&mut domain).await?;
            },
            0x04 => { // IPv6
                let mut addr = [0u8; 16];
                upstream.read_exact(&mut addr).await?;
            },
            _ => return Err(anyhow::anyhow!("上游代理返回了不支持的地址类型")),
        }
        let mut port = [0u8; 2];
        upstream.read_exact(&mut port).await?;
        
        // 11. 发送成功响应给客户端
        let response = [
            0x05, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        inbound_writer.write_all(&response).await?;
        
        // 12. 双向转发数据
        let (mut upstream_reader, mut upstream_writer) = upstream.into_split();
        let client_to_proxy = tokio::io::copy(&mut inbound_reader, &mut upstream_writer);
        let proxy_to_client = tokio::io::copy(&mut upstream_reader, &mut inbound_writer);
        
        tokio::select! {
            res = client_to_proxy => {
                if let Err(e) = res {
                    error!("客户端到代理传输错误: {}", e);
                }
            },
            res = proxy_to_client => {
                if let Err(e) = res {
                    error!("代理到客户端传输错误: {}", e);
                }
            }
        }
        
        Ok(())
    }
}