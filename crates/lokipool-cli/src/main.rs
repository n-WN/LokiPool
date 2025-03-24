use anyhow::Result;
use lokipool_core::{Config, Pool, PoolOptions, init_logger};
use tracing::{info, error};
use std::path::Path;
use colored::*;

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
                error!("加载配置失败: {}", e);
                Config::default()
            }
        }
    } else {
        info!("使用默认配置");
        let default_config = Config::default();
        if let Err(e) = default_config.save_to_file(config_path) {
            error!("保存默认配置失败: {}", e);
        } else {
            info!("默认配置已保存到 {}", config_path.display());
        }
        default_config
    };
    
    // 创建池选项
    let pool_options = PoolOptions::from_config(&config);
    
    // 创建代理池
    let pool = Pool::new_with_proxies(config.proxies.clone(), pool_options);
    
    // 测试所有代理
    info!("开始测试代理...");
    let test_results = pool.test_all().await;
    
    // 显示测试结果
    for (proxy_config, result) in test_results {
        if result.success {
            println!("{} {}:{} - {}ms", 
                "✓".green().bold(),
                proxy_config.host,
                proxy_config.port,
                result.latency.unwrap_or(0)
            );
        } else {
            println!("{} {}:{} - {}", 
                "✗".red().bold(),
                proxy_config.host,
                proxy_config.port,
                result.error.unwrap_or_else(|| "未知错误".to_string())
            );
        }
    }
    
    // 这里可以添加更多功能，例如交互式命令行界面
    
    info!("LokiPool CLI 已退出");
    Ok(())
}
