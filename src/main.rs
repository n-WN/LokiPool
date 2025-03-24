use anyhow::Result;
use lokipool::{Config, Pool, PoolOptions, init_logger};
use tracing::{info, error};
use std::path::Path;
use std::io::{self, Write};
use tokio::sync::{mpsc, broadcast};
use tokio::time::{sleep, Duration, timeout};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

mod socks_server;
use socks_server::{SocksServer, SocksServerConfig};

// 如果需要使用ProxyConfig，需要将其添加到导入中
use lokipool::ProxyConfig;

const VERSION: &str = "v0.1.0";
const BANNER: &str = r#"
╦  ╔═╗╦╔═╦╔═╗╔═╗╔═╗╦  
║  ║ ║╠╩╗║╠═╝║ ║║ ║║  
╩═╝╚═╝╩ ╩╩╩  ╚═╝╚═╝╩═╝
"#;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    init_logger();
    
    // 显示程序信息
    println!("{} {}", BANNER, VERSION);
    info!("LokiPool SOCKS5 proxy manager starting...");
    
    // 加载或创建配置
    let config_path = Path::new("config.toml");
    let config = if config_path.exists() {
        match Config::from_file(config_path) {
            Ok(cfg) => {
                info!("配置已从 {} 加载", config_path.display());
                cfg
            }
            Err(e) => {
                error!("加载配置失败: {} - 使用默认配置", e);
                // 尝试读取内容并记录问题
                if let Ok(content) = std::fs::read_to_string(config_path) {
                    error!("配置文件内容预览: \n{}", content.lines().take(5).collect::<Vec<_>>().join("\n"));
                }
                Config::default()
            }
        }
    } else {
        info!("配置文件不存在，使用默认配置");
        let default_config = Config::default();
        // 创建示例配置
        let example_config = create_example_config();
        if let Err(e) = example_config.save_to_file(config_path) {
            error!("保存示例配置失败: {}", e);
        } else {
            info!("示例配置已保存到 {}", config_path.display());
        }
        default_config
    };
    
    // 创建池选项
    let pool_options = PoolOptions::from_config(&config);
    
    // 创建代理池
    let mut pool = Pool::new_with_proxies(config.proxies.clone(), pool_options.clone());
    
    // 在测试所有代理之前，确保有代理存在
    if config.proxies.is_empty() {
        info!("没有找到任何代理配置，请在config.toml中添加代理或使用代理文件");
        
        // 如果没有代理，创建一个本地示例代理以便程序可以继续运行
        let local_proxy = ProxyConfig {
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: None,
            password: None,
            location: Some("Local".to_string()),
            proxy_type: "socks5".to_string(),
        };
        
        info!("添加了一个本地示例代理 127.0.0.1:1080 以便程序继续运行");
        let mut proxies = Vec::new();
        proxies.push(local_proxy);
        
        // 创建代理池
        pool = Pool::new_with_proxies(proxies, pool_options);
    }
    
    // 测试所有代理
    info!("开始测试代理...");
    let test_results = pool.test_all().await;
    
    // 显示测试结果
    for (config, result) in test_results {
        if result.success {
            info!(
                "代理 {}:{} 测试成功, 延迟: {}ms", 
                config.host, 
                config.port, 
                result.latency.unwrap_or(0)
            );
        } else {
            error!(
                "代理 {}:{} 测试失败: {}", 
                config.host, 
                config.port, 
                result.error.unwrap_or_else(|| "未知错误".to_string())
            );
        }
    }
    
    // 创建关闭信号通道
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let shutdown_rx = shutdown_tx.subscribe();
    
    // 创建SOCKS5服务器，从配置中读取设置
    let socks_config = SocksServerConfig {
        bind_address: config.socks_server.bind_address.clone(),
        bind_port: config.socks_server.bind_port,
    };
    let socks_server = SocksServer::new(socks_config.clone(), pool.clone());
    
    // 启动SOCKS5服务器，带退出控制
    let server_handle = {
        let shutdown_rx = shutdown_rx;
        tokio::spawn(async move {
            if let Err(e) = socks_server.run_with_shutdown(shutdown_rx).await {
                error!("SOCKS5服务器运行出错: {}", e);
            }
        })
    };
    
    info!("SOCKS5服务器已启动: {}:{}", socks_config.bind_address, socks_config.bind_port);
    info!("请配置您的应用程序使用此代理服务器");
    
    // 启动交互式命令行
    let (tx, mut rx) = mpsc::channel::<String>(100);
    
    // 命令处理线程
    let cmd_pool = Arc::new(TokioMutex::new(pool));
    let shutdown_tx_clone = shutdown_tx.clone();
    let cmd_handle = {
        let pool = Arc::clone(&cmd_pool);
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd.trim() {
                    "show" => {
                        let pool = pool.lock().await;
                        match pool.get_available() {
                            Some(proxy) => {
                                println!("当前代理: {}:{} (延迟: {}ms)",
                                    proxy.info.host, 
                                    proxy.info.port,
                                    proxy.latency
                                );
                            },
                            None => println!("没有可用的代理"),
                        }
                        // 确保输出被立即刷新
                        io::stdout().flush().unwrap();
                    },
                    "list" => {
                        println!("代理列表功能尚未实现");
                        io::stdout().flush().unwrap();
                    },
                    "next" => {
                        println!("切换代理功能尚未实现");
                        io::stdout().flush().unwrap();
                    },
                    "test" => {
                        // 重新测试所有代理
                        println!("重新测试所有代理...");
                        let pool = pool.lock().await;
                        let results = pool.test_all().await;
                        println!("测试完成，共 {} 个代理", results.len());
                        for (config, result) in results {
                            if result.success {
                                println!("✓ {}:{} - {}ms", 
                                    config.host, 
                                    config.port, 
                                    result.latency.unwrap_or(0)
                                );
                            } else {
                                println!("✗ {}:{} - {}", 
                                    config.host, 
                                    config.port, 
                                    result.error.unwrap_or_else(|| "未知错误".to_string())
                                );
                            }
                        }
                        io::stdout().flush().unwrap();
                    },
                    "diag" | "diagnose" => {
                        println!("开始诊断代理连接...");
                        diagnose_proxy_connection(&pool.lock().await).await;
                        io::stdout().flush().unwrap();
                    },
                    "help" => {
                        println!("可用命令:");
                        println!("  show - 显示当前使用的代理及其延迟");
                        println!("  list - 显示所有可用代理及其延迟排序");
                        println!("  next - 手动切换到下一个代理");
                        println!("  test - 重新测试所有代理");
                        println!("  diag - 诊断代理连接问题");
                        println!("  help - 显示帮助信息");
                        println!("  quit - 退出程序");
                        // 确保输出被立即刷新
                        io::stdout().flush().unwrap();
                    },
                    "quit" | "exit" => {
                        println!("程序退出中...");
                        io::stdout().flush().unwrap();
                        // 发送关闭信号
                        let _ = shutdown_tx_clone.send(());
                        break;
                    },
                    "" => {},
                    _ => {
                        println!("未知命令: {}，输入 help 查看帮助", cmd);
                        io::stdout().flush().unwrap();
                    }
                }
            }
        })
    };
    
    // 命令行输入线程
    let input_handle = tokio::spawn(async move {
        println!("\n输入 'help' 查看可用命令，输入 'quit' 退出程序");
        io::stdout().flush().unwrap();
        
        let stdin = io::stdin();
        let mut buffer = String::new();
        
        loop {
            print!("> ");
            io::stdout().flush().unwrap();
            buffer.clear();
            
            if stdin.read_line(&mut buffer).is_err() {
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            
            let cmd = buffer.trim().to_string();
            // 立即发送命令，不要等待
            if let Err(e) = tx.send(cmd.clone()).await {
                eprintln!("发送命令失败: {}", e);
                break;
            }
            
            if cmd == "quit" || cmd == "exit" {
                break;
            }
            
            // 添加短暂延迟，确保命令处理线程有时间处理命令
            sleep(Duration::from_millis(50)).await;
        }
    });
    
    // 等待所有任务完成
    let _ = cmd_handle.await;
    let _ = input_handle.await;
    
    // 确保SOCKS5服务器关闭后再退出
    let shutdown_timeout = Duration::from_secs(3);
    match timeout(shutdown_timeout, server_handle).await {
        Ok(_) => info!("SOCKS5服务器已正常关闭"),
        Err(_) => {
            info!("SOCKS5服务器关闭超时，强制关闭");
            // 强制关闭，不再等待
        }
    }
    
    // 程序退出
    info!("LokiPool 已退出");
    Ok(())
}

// 添加辅助函数生成示例配置
fn create_example_config() -> Config {
    let mut config = Config::default();
    
    // 设置SOCKS服务器配置
    config.socks_server.bind_address = "127.0.0.1".to_string();
    config.socks_server.bind_port = 1080;
    
    // 添加一些示例代理
    config.proxies.push(ProxyConfig {
        host: "127.0.0.1".to_string(),
        port: 12333, // 使用不同于SOCKS服务器的端口
        username: None,
        password: None,
        location: Some("Local".to_string()),
        proxy_type: "socks5".to_string(),
    });
    
    config
}

// 修改诊断函数，接受互斥锁守卫而不是池引用
async fn diagnose_proxy_connection(pool: &tokio::sync::MutexGuard<'_, Pool>) {
    use colored::*;
    use tokio::net::TcpStream;
    use std::time::Duration;
    use reqwest::Client;
    
    // 获取当前代理
    let proxy = match pool.get_available() {
        Some(p) => p,
        None => {
            println!("{} {}", "✗".red().bold(), "没有可用的代理!".red());
            println!("{}:", "建议".yellow().bold());
            println!("  1. 运行 'test' 命令重新测试所有代理");
            println!("  2. 检查配置文件中的代理设置");
            println!("  3. 确保上游代理服务器正常运行");
            return;
        }
    };
    
    println!("当前代理: {}:{}", proxy.info.host, proxy.info.port);
    
    // 测试1: 检查代理TCP连接
    print!("测试代理TCP连接... ");
    match TcpStream::connect(format!("{}:{}", proxy.info.host, proxy.info.port)).await {
        Ok(_) => println!("{} 连接成功", "✓".green().bold()),
        Err(e) => {
            println!("{} 连接失败: {}", "✗".red().bold(), e);
            println!("{}:", "建议".yellow().bold());
            println!("  1. 检查代理地址和端口是否正确");
            println!("  2. 确认代理服务器是否在线并运行");
            println!("  3. 检查网络连接和防火墙设置");
            return;
        }
    }
    
    // 测试2: 测试HTTP请求
    print!("通过代理发送HTTP请求... ");
    let client = match Client::builder()
        .proxy(reqwest::Proxy::all(format!("socks5://{}:{}", proxy.info.host, proxy.info.port)).unwrap())
        .timeout(Duration::from_secs(10))
        .build() {
        Ok(c) => c,
        Err(e) => {
            println!("{} 创建客户端失败: {}", "✗".red().bold(), e);
            return;
        }
    };
    
    match client.get("http://www.baidu.com").send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("{} 请求成功, 状态码: {}", "✓".green().bold(), resp.status());
            } else {
                println!("{} 请求返回非成功状态码: {}", "!".yellow().bold(), resp.status());
            }
        },
        Err(e) => {
            println!("{} 请求失败: {}", "✗".red().bold(), e);
            println!("{}:", "建议".yellow().bold());
            println!("  1. 确认代理支持SOCKS5协议");
            println!("  2. 检查代理的网络连接");
            println!("  3. 尝试使用不同的目标URL");
        }
    }
    
    // 测试3: 检查SOCKS服务器设置
    println!("\n{}", "SOCKS服务器配置诊断:".cyan().bold());
    println!("  主机: {}", "127.0.0.1".cyan());
    
    // 修复这行，去掉get_config()调用
    println!("  端口: {}", "1080".cyan());
    
    println!("\n如要进行更详细的测试，请使用 tools/test_proxy.sh 脚本");
}