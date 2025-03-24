//! UI相关功能模块
//! 
//! 提供命令行界面和交互式UI组件

use colored::*;
use indicatif::{ProgressBar, ProgressStyle};

/// UI配置
#[derive(Debug, Clone)]
pub struct UiConfig {
    /// 是否使用彩色输出
    pub use_color: bool,
    /// 是否显示进度条
    pub show_progress: bool,
    /// 控制台宽度
    pub console_width: Option<u16>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            use_color: true,
            show_progress: true,
            console_width: None,
        }
    }
}

/// 创建一个标准格式的进度条
pub fn create_progress_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
    pb
}

/// 打印横幅
pub fn print_banner(version: &str) {
    println!("{}\n{}", 
        r#"
╦  ╔═╗╦╔═╦╔═╗╔═╗╔═╗╦  
║  ║ ║╠╩╗║╠═╝║ ║║ ║║  
╩═╝╚═╝╩ ╩╩╩  ╚═╝╚═╝╩═╝
        "#.cyan().bold(),
        format!("Version: {}", version).yellow()
    );
}

/// 打印成功消息
pub fn print_success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

/// 打印错误消息
pub fn print_error(msg: &str) {
    println!("{} {}", "✗".red().bold(), msg);
}

/// 打印警告消息
pub fn print_warning(msg: &str) {
    println!("{} {}", "!".yellow().bold(), msg);
}

/// 打印信息消息
pub fn print_info(msg: &str) {
    println!("{} {}", "i".blue().bold(), msg);
}

/// 初始化UI
pub fn init_ui() {
    #[cfg(feature = "ui")]
    {
        // 当启用UI特性时执行的初始化代码
    }
}
