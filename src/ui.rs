use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
use native_dialog::{FileDialog, MessageDialog, MessageType};
use std::path::PathBuf;

/// 显示欢迎信息
pub fn welcome() {
        println!();
        println!("{}", style("╔══════════════════════════════════════════╗").cyan());
        println!("{}", style("║           ABU - Alpha Bank Union         ║").cyan());
        println!("{}", style("║            Alpha Coin 钱包工具           ║").cyan());
        println!("{}", style("║   ©2026 Max Shin - All Rights Reserved.  ║").cyan());
        println!("{}", style("╚══════════════════════════════════════════╝").cyan());
        println!();
        println!("欢迎使用 Alpha Bank Union Alpha Coin钱包工具");
        println!("此工具用于管理您的Alpha Coin钱包");
        println!();
    }

/// 主菜单
pub fn main_menu() -> usize {
    let items = vec![
        "生成并导出交易 (导出ZIP)",
        "导入并验证交易 (导入ZIP)",
        "导出反假币证据 (导出JSON)",
        "导入ABU交易黑名单 (导入JSON)",
        "查看交易简图",
        "清理缓存 (保留钱包数据库)",
        "退出",
    ];
    Select::with_theme(&ColorfulTheme::default())
        .with_prompt("请选择操作")
        .items(&items)
        .default(0)
        .interact()
        .unwrap()
}

pub fn choose_open_file(title: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
    let mut dialog = FileDialog::new();
    dialog = dialog.set_title(title);
    for (desc, exts) in filters {
        dialog = dialog.add_filter(*desc, exts);
    }
    dialog.show_open_single_file().ok().flatten()
}

pub fn choose_save_file(title: &str, default_name: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
    let mut dialog = FileDialog::new();
    dialog = dialog.set_title(title);
    dialog = dialog.set_filename(default_name);
    for (desc, exts) in filters {
        dialog = dialog.add_filter(*desc, exts);
    }
    dialog.show_save_single_file().ok().flatten()
}

/// 输入密码（隐藏）
pub fn input_password(prompt: &str) -> String {
    Password::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .interact()
        .unwrap()
}

/// 输入文本
pub fn input_text(prompt: &str, default: &str) -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default.to_string())
        .interact_text()
        .unwrap()
}

/// 确认对话框
pub fn confirm(prompt: &str) -> bool {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(true)
        .interact()
        .unwrap()
}

/// 消息弹窗
pub fn message_info(title: &str, msg: &str) {
    MessageDialog::new()
        .set_type(MessageType::Info)
        .set_title(title)
        .set_text(msg)
        .show_alert()
        .unwrap();
}

/// 错误弹窗
pub fn message_error(title: &str, msg: &str) {
    MessageDialog::new()
        .set_type(MessageType::Error)
        .set_title(title)
        .set_text(msg)
        .show_alert()
        .unwrap();
}