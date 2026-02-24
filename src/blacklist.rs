//! ABU 黑名单导入与查询

use anyhow::Result;  // 去掉 anyhow
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct BlacklistEntry {
    pub address: String,
    #[allow(dead_code)]
    pub reason: String,
    #[allow(dead_code)]
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct Blacklist {
    #[allow(dead_code)]
    pub version: String,
    pub entries: Vec<BlacklistEntry>,
}

/// 从 JSON 文件导入黑名单
pub fn import_blacklist(path: &Path) -> Result<Blacklist> {
    let data = fs::read_to_string(path)?;
    let list: Blacklist = serde_json::from_str(&data)?;
    Ok(list)
}

/// 检查地址是否在黑名单中
pub fn is_blacklisted(addr: &str, blacklist: &Blacklist) -> bool {
    blacklist.entries.iter().any(|e| e.address == addr)
}