//! ZIP 打包和解包

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;  // 只保留 Path
use zip::{write::FileOptions, ZipArchive, ZipWriter};

/// 将交易包打包为 ZIP，包含交易 JSON 和发送方公钥
pub fn create_transaction_zip(
    tx_json: &str,
    pubkey_bytes: &[u8],
    pubkey_filename: &str,
    zip_path: &Path,
) -> Result<()> {
    let file = File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

    // 添加交易 JSON
    zip.start_file("transaction.json", options)?;
    zip.write_all(tx_json.as_bytes())?;

    // 添加公钥
    zip.start_file(pubkey_filename, options)?;
    zip.write_all(pubkey_bytes)?;

    zip.finish()?;
    Ok(())
}

/// 解压 ZIP，返回 (交易 JSON 内容, 公钥内容, 公钥文件名)
pub fn extract_transaction_zip(zip_path: &Path) -> Result<(String, Vec<u8>, String)> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut tx_json = None;
    let mut pubkey_bytes = None;
    let mut pubkey_filename = None;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name == "transaction.json" {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            tx_json = Some(content);
        } else if name.ends_with(".asc") {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            pubkey_bytes = Some(buf);
            pubkey_filename = Some(name);
        }
    }

    let tx_json = tx_json.context("ZIP 中缺少 transaction.json")?;
    let pubkey_bytes = pubkey_bytes.context("ZIP 中缺少公钥文件")?;
    let pubkey_filename = pubkey_filename.context("ZIP 中公钥文件名无效")?;

    Ok((tx_json, pubkey_bytes, pubkey_filename))
}