//! ABU 钱包工具主入口

mod crypto;
mod db;
mod transaction;
mod zip_util;
mod blacklist;
mod config;
mod ui;

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use simple_home_dir::home_dir;
use std::fs;
use std::path::PathBuf;  // 去掉 Path
use sequoia_openpgp as openpgp;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// 应用状态
struct App {
    db_path: PathBuf,
    trusted_pubkeys_dir: PathBuf,
    my_privkey_path: Option<PathBuf>,
    my_pubkey_path: Option<PathBuf>,
    my_identity: Option<(String, String)>, // (name, email)
    my_pubkey_bytes: Option<Vec<u8>>,
    private_key: Option<crypto::SecurePrivateKey>,
    config: config::Config,
    blacklist: Option<blacklist::Blacklist>,
}

impl App {
    fn new() -> Result<Self> {
        let home = home_dir().ok_or_else(|| anyhow!("无法获取用户目录"))?;
        let abu_dir = home.join(".abu");
        if !abu_dir.exists() {
            fs::create_dir_all(&abu_dir)?;
        }
        let db_path = abu_dir.join("personal_wallet.db");
        let trusted_pubkeys_dir = abu_dir.join("trusted_pubkeys");
        if !trusted_pubkeys_dir.exists() {
            fs::create_dir_all(&trusted_pubkeys_dir)?;
        }
        let config = config::Config::load();

        Ok(Self {
            db_path,
            trusted_pubkeys_dir,
            my_privkey_path: None,
            my_pubkey_path: None,
            my_identity: None,
            my_pubkey_bytes: None,
            private_key: None,
            config,
            blacklist: None,
        })
    }

    /// 初始化个人密钥（首次运行或更换密钥）
    fn setup_my_keys(&mut self) -> Result<()> {
        ui::message_info("设置个人密钥", "请选择您的私钥文件 (.bin)");
        let priv_filter = [("私钥文件", &["bin"][..])];
        let priv_path = ui::choose_open_file("选择私钥", &priv_filter)
            .ok_or_else(|| anyhow!("未选择私钥"))?;

        ui::message_info("选择公钥", "请选择对应的公钥文件 (.asc)");
        let pub_filter = [("公钥文件", &["asc"][..])];
        let pub_path = ui::choose_open_file("选择公钥", &pub_filter)
            .ok_or_else(|| anyhow!("未选择公钥"))?;

        // 输入密码解密私钥
        let password = ui::input_password("请输入私钥密码");
        let priv_bytes = crypto::decrypt_private_key(&priv_path, &password)?;

        // 解析公钥获取身份
        let (name, email, _cert) = crypto::parse_pubkey_file(&pub_path)?;

        // 保存到应用目录
        let home = home_dir().unwrap();
        let target_priv = home.join(".abu/my_secret_key.bin");
        fs::copy(&priv_path, &target_priv)?;
        // 公钥保留原位置？我们也可以复制一份，但用户可能希望保留原位置，所以我们只记录路径
        // 为了方便，我们将公钥也复制到 .abu 下
        let target_pub = home.join(".abu/my_public_key.asc");
        fs::copy(&pub_path, &target_pub)?;

        self.my_privkey_path = Some(target_priv.clone());
        self.my_pubkey_path = Some(target_pub.clone());
        self.my_identity = Some((name, email));
        self.private_key = Some(crypto::SecurePrivateKey::new(priv_bytes));
        // 读取公钥字节
        self.my_pubkey_bytes = Some(fs::read(&target_pub)?);

        // 保存配置
        self.config.last_my_privkey = Some(target_priv);
        self.config.last_my_pubkey = Some(target_pub);
        self.config.save()?;

        ui::message_info("成功", "密钥已导入并保存至 .abu 目录");
        Ok(())
    }

    /// 确保已加载密钥，否则提示导入
    fn ensure_keys(&mut self) -> Result<()> {
        if self.private_key.is_none() {
            // 尝试从默认位置加载
            let home = home_dir().unwrap();
            let priv_path = home.join(".abu/my_secret_key.bin");
            let pub_path = home.join(".abu/my_public_key.asc");
            if priv_path.exists() && pub_path.exists() {
                // 自动加载，但需要密码
                let password = ui::input_password("请输入私钥密码以解锁");
                let priv_bytes = crypto::decrypt_private_key(&priv_path, &password)?;
                let (name, email, _cert) = crypto::parse_pubkey_file(&pub_path)?;
                self.my_privkey_path = Some(priv_path);
                self.my_pubkey_path = Some(pub_path.clone());
                self.my_identity = Some((name, email));
                self.private_key = Some(crypto::SecurePrivateKey::new(priv_bytes));
                self.my_pubkey_bytes = Some(fs::read(&pub_path)?);
            } else {
                ui::message_info("首次使用", "请先导入您的私钥和公钥");
                self.setup_my_keys()?;
            }
        }
        Ok(())
    }

    /// 获取对方公钥（从本地信任目录或让用户选择）
    fn get_peer_pubkey(&self, peer_id: &str) -> Result<Vec<u8>> {
        // 先尝试从 trusted_pubkeys_dir 中查找匹配的文件名（包含 peer_id 的 .asc）
        if let Ok(entries) = fs::read_dir(&self.trusted_pubkeys_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("asc") {
                    if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                        if name.contains(peer_id) {
                            return fs::read(&path).context("读取公钥失败");
                        }
                    }
                }
            }
        }
        // 否则让用户选择
        let filters = [("公钥文件", &["asc"][..])];
        let path = ui::choose_open_file(&format!("选择 {} 的公钥", peer_id), &filters)
            .ok_or_else(|| anyhow!("未选择公钥"))?;
        fs::read(&path).context("读取公钥失败")
    }

    /// 生成并发送交易
    fn send_transaction(&mut self) -> Result<()> {
        self.ensure_keys()?;

        // 输入交易信息
        let to_id = ui::input_text("对方游戏ID", "");
        let to_email = ui::input_text("对方邮箱", "");
        let to_addr = format!("{} <{}>", to_id, to_email);
        let amount: f64 = ui::input_text("金额", "0.0").parse()?;
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // 生成交易ID（简单用时间戳+随机）
        let tx_id = format!("TX-{}", Local::now().format("%Y%m%d%H%M%S"));

        // 创建交易对象
        let mut tx = transaction::Transaction {
            id: tx_id.clone(),
            type_: "common".to_string(),
            from: format!("{} <{}>", self.my_identity.as_ref().unwrap().0, self.my_identity.as_ref().unwrap().1),
            to: to_addr,
            amount,
            timestamp: timestamp.clone(),
            hash: "".to_string(),
            from_signature: "".to_string(),
            to_signature: None,
            status: "pending".to_string(),
        };
        tx.hash = tx.calculate_hash();

        // 签名（用私钥）
        let msg = format!("{}:{}", tx.id, tx.amount);
        let sig = self.private_key.as_ref().unwrap().use_key(|key| {
            crypto::sign_message(key, msg.as_bytes()).unwrap()
        });
        tx.from_signature = STANDARD.encode(sig);

        // 打包为 ZIP
        let tx_json = serde_json::to_string_pretty(&tx)?;
        let pubkey_bytes = self.my_pubkey_bytes.as_ref().unwrap();
        let pubkey_filename = format!("{}.asc", self.my_identity.as_ref().unwrap().0);

        let default_zip = format!("{}.zip", tx_id);
        let filters = [("ZIP 文件", &["zip"][..])];
        let save_path = ui::choose_save_file("保存交易包", &default_zip, &filters)
            .ok_or_else(|| anyhow!("未选择保存位置"))?;

        zip_util::create_transaction_zip(&tx_json, pubkey_bytes, &pubkey_filename, &save_path)?;

        // 暂存交易到数据库（待确认）
        let conn = db::init_db(&self.db_path)?;
        db::insert_transaction(&conn, &tx)?;

        ui::message_info("成功", &format!("交易包已保存至 {:?}\n请将此文件发送给收款方。", save_path));
        Ok(())
    }

    /// 导入并验证交易
    fn receive_transaction(&mut self) -> Result<()> {
        self.ensure_keys()?;

        let filters = [("ZIP 文件", &["zip"][..])];
        let zip_path = ui::choose_open_file("选择交易包", &filters)
            .ok_or_else(|| anyhow!("未选择文件"))?;

        // 解压
        let (tx_json, peer_pubkey_bytes, pubkey_filename) = zip_util::extract_transaction_zip(&zip_path)?;
        let tx: transaction::Transaction = serde_json::from_str(&tx_json)?;

        // 保存对方公钥到 trusted_pubkeys_dir
        let target_pub = self.trusted_pubkeys_dir.join(&pubkey_filename);
        fs::write(&target_pub, &peer_pubkey_bytes)?;

        // 验证交易哈希
        if tx.hash != tx.calculate_hash() {
            return Err(anyhow!("交易哈希不匹配，可能被篡改"));
        }

        // 验证对方签名（付款方）
        // 需要从对方公钥中解析出公钥字节
        // 这里简化：用 sequoia 从公钥文件提取 Ed25519 公钥字节（需要实现）
        // 假设有一个函数 extract_ed25519_pubkey_from_asc
        let peer_pubkey_bytes = extract_ed25519_pubkey_from_asc(&peer_pubkey_bytes)?;
        if !crypto::verify_signature(&peer_pubkey_bytes, format!("{}:{}", tx.id, tx.amount).as_bytes(), &STANDARD.decode(&tx.from_signature)?) {
            return Err(anyhow!("付款方签名验证失败"));
        }

        // 可选：检查黑名单
        if let Some(bl) = &self.blacklist {
            if blacklist::is_blacklisted(&tx.from, bl) {
                ui::message_error("警告", "付款方地址在黑名单中，请谨慎确认！");
                if !ui::confirm("仍要接收此交易吗？") {
                    return Ok(());
                }
            }
        }

        // 检查余额是否足够（需要查询对方账本？这里我们只能信任对方，但可做本地检查）
        // 因为是本地账本，我们没有对方余额，只能依赖后续ABU仲裁。此处仅提醒。
        ui::message_info("提醒", "本地无法验证对方余额，请自行判断风险。");

        // 生成收款方签名（确认）
        let ack_msg = format!("ACK:{}", tx.id);
        let my_sig = self.private_key.as_ref().unwrap().use_key(|key| {
            crypto::sign_message(key, ack_msg.as_bytes()).unwrap()
        });
        let mut confirmed_tx = tx.clone();
        confirmed_tx.to_signature = Some(STANDARD.encode(my_sig));
        confirmed_tx.status = "completed".to_string();

        // 写入本地数据库
        let conn = db::init_db(&self.db_path)?;
        db::insert_transaction(&conn, &confirmed_tx)?;

        ui::message_info("成功", "交易已接收并写入账本");
        Ok(())
    }

    /// 导出反假币证据（全部交易JSON）
    fn export_evidence(&self) -> Result<()> {
        let conn = db::init_db(&self.db_path)?;
        let txs = db::get_all_transactions(&conn)?;
        let json = serde_json::to_string_pretty(&txs)?;
        let filters = [("JSON 文件", &["json"][..])];
        let save_path = ui::choose_save_file("保存证据文件", "evidence.json", &filters)
            .ok_or_else(|| anyhow!("未选择保存位置"))?;
        fs::write(save_path, json)?;
        ui::message_info("成功", "证据文件已导出");
        Ok(())
    }

    /// 导入ABU黑名单
    fn import_blacklist(&mut self) -> Result<()> {
        let filters = [("JSON 文件", &["json"][..])];
        let path = ui::choose_open_file("选择黑名单JSON", &filters)
            .ok_or_else(|| anyhow!("未选择文件"))?;
        let bl = blacklist::import_blacklist(&path)?;
        self.blacklist = Some(bl);
        ui::message_info("成功", &format!("已导入黑名单，共 {} 条", self.blacklist.as_ref().unwrap().entries.len()));
        Ok(())
    }

    /// 查看交易简图（简单打印）
    fn view_chart(&mut self) -> Result<()> {
    // 确保密钥已加载（否则无法获取身份信息）
    self.ensure_keys()?;

    let conn = db::init_db(&self.db_path)?;
    let txs = db::get_all_transactions(&conn)?;

    // 安全获取身份信息，避免 panic
    let identity = self.my_identity.as_ref()
        .ok_or_else(|| anyhow!("未找到身份信息，请先导入密钥"))?;

    let balance = db::get_balance(&conn, &identity.0)?;

    println!("\n===== 交易简图 =====");
    println!("当前余额: {}", balance);
    println!("交易记录:");
    for tx in txs.iter().rev().take(10) {
        println!("{} {} -> {} : {} ({})", 
            tx.timestamp, tx.from, tx.to, tx.amount, tx.status);
    }
    println!("===================\n");

    ui::confirm("按回车继续...");
    Ok(())
    }
    
    /// 清理缓存（删除 trusted_pubkeys 和配置文件，保留数据库和私钥）
    fn clean_cache(&self) -> Result<()> {
        if !ui::confirm("此操作将删除所有信任的公钥文件和配置文件，但保留数据库和私钥。确定？") {
            return Ok(());
        }
        // 删除 trusted_pubkeys 目录下所有文件
        if self.trusted_pubkeys_dir.exists() {
            for entry in fs::read_dir(&self.trusted_pubkeys_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        // 删除配置文件
        let home = home_dir().unwrap();
        let config_path = home.join(".abu/last_settings.ini");
        if config_path.exists() {
            fs::remove_file(config_path)?;
        }
        ui::message_info("成功", "缓存已清理");
        Ok(())
    }
}

/// 辅助函数：从 armored 公钥中提取 Ed25519 公钥字节
fn extract_ed25519_pubkey_from_asc(asc_data: &[u8]) -> Result<Vec<u8>> {
    use openpgp::parse::Parse;
    use openpgp::policy::StandardPolicy;
    use openpgp::types::PublicKeyAlgorithm;
    use openpgp::crypto::mpi::PublicKey;  // 导入 PublicKey 枚举
    use std::io::Cursor;

    let cert = openpgp::Cert::from_reader(Cursor::new(asc_data))?;
    let p = &StandardPolicy::new();

    for ka in cert.keys().with_policy(p, None) {
        let key = ka.key();
        if key.pk_algo() == PublicKeyAlgorithm::EdDSA {
            // mpis() 返回 &PublicKey 枚举
            match key.mpis() {
                PublicKey::EdDSA { q, .. } => {
                    let bytes = q.value();
                    if bytes.len() >= 32 {
                        return Ok(bytes.to_vec());
                    }
                }
                _ => continue, // 理论上不会发生，因为算法已匹配
            }
        }
    }
    Err(anyhow!("未找到 Ed25519 公钥"))
}

fn main() -> Result<()> {
    ui::welcome();
    let mut app = App::new()?;

    // 首次运行提示导入黑名单
    if app.blacklist.is_none() && ui::confirm("是否立即导入ABU黑名单？") {
        if let Err(e) = app.import_blacklist() {
            ui::message_error("错误", &format!("导入失败: {}", e));
        }
    }

    loop {
        match ui::main_menu() {
            0 => {
                if let Err(e) = app.send_transaction() {
                    ui::message_error("错误", &format!("发送失败: {}", e));
                }
            }
            1 => {
                if let Err(e) = app.receive_transaction() {
                    ui::message_error("错误", &format!("接收失败: {}", e));
                }
            }
            2 => {
                if let Err(e) = app.export_evidence() {
                    ui::message_error("错误", &format!("导出失败: {}", e));
                }
            }
            3 => {
                if let Err(e) = app.import_blacklist() {
                    ui::message_error("错误", &format!("导入失败: {}", e));
                }
            }
            4 => {
                if let Err(e) = app.view_chart() {
                    ui::message_error("错误", &format!("查看失败: {}", e));
                }
            }
            5 => {
                if let Err(e) = app.clean_cache() {
                    ui::message_error("错误", &format!("清理失败: {}", e));
                }
            }
            6 => break,
            _ => {}
        }
    }

    Ok(())
}