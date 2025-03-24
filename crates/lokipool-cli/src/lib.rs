//! LokiPool CLI - Command Line Interface for LokiPool SOCKS5 proxy manager
//! 
//! This library provides the CLI functionality for managing LokiPool.

/// CLI命令枚举
#[derive(Debug, Clone)]
pub enum Command {
    /// 显示所有代理
    List,
    /// 显示当前代理
    Show,
    /// 切换到下一个代理
    Next,
    /// 测试所有代理
    Test,
    /// 退出程序
    Quit,
}

/// CLI配置
#[derive(Debug, Clone)]
pub struct CliConfig {
    /// 是否显示横幅
    pub show_banner: bool,
    /// 是否启用彩色输出
    pub colored_output: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            show_banner: true,
            colored_output: true,
        }
    }
}
