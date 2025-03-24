use anyhow::Result;
use lokipool_core::{Config, Pool, PoolOptions, init_logger};
use lokipool_api::{ApiServer, ApiConfig};
use tracing::{info, error};
use std::path::Path;

const VERSION: &str = "v0.1.0";

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    init_logger();
    
    info!("LokiPool API Server starting... {}", VERSION);
    
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
    
    // 创建API配置
    let api_config = ApiConfig::default();
    
    // 创建并运行API服务器
    let api_server = ApiServer::new(pool, config, api_config);
    
    // 运行API服务器
    info!("启动API服务器...");
    api_server.run().await?;
    
    info!("LokiPool API Server 已停止");
    Ok(())
}
