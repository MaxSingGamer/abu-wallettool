 密钥处理：解密私钥、解析公钥、签名、哈希

use anyhow::{anyhow, Result};  // 删除 Context
use ring::signature::{self, Ed25519KeyPair};  // 只保留需要的
use sequoia_openpgp as openpgp;
use openpgp::Cert;
use openpgp::parse::Parse;
use sha3::{Digest, Sha3_256};
use zeroize::{Zeroize, ZeroizeOnDrop};
use std::fs;
use std::path::Path;
use std::io::Cursor;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

// ---------- 公钥解析 ----------
/// 从 armored 公钥文件解析出 Cert，并提取第一个 UserID（格式 "Name <email>"）
pub fn parse_pubkey_file(path: &Path) -> Result<(String, String, Cert)> {
    let data = fs::read_to_string(path)?;
    let cert = Cert::from_reader(Cursor::new(data))?;
    let userid = cert
        .userids()
        .next()
        .ok_or_else(|| anyhow!("公钥中没有 UserID"))?;
    let userid_str = userid.userid().to_string();
    // 预期格式 "Name <email>"，简单拆解
    let (name, email) = if let Some((n, e)) = userid_str.split_once('<') {
        let name = n.trim().to_string();
        let email = e.trim_end_matches('>').to_string();
        (name, email)
    } else {
        (userid_str.clone(), "".to_string())
    };
    Ok((name, email, cert))
}

// ---------- 私钥解密 ----------
/// 解密 keygentool 生成的私钥（格式：salt(16) + nonce(12) + ciphertext）
pub fn decrypt_private_key(
    encrypted_path: &Path,
    password: &str,
) -> Result<Vec<u8>> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    use pbkdf2::pbkdf2;
    use hmac::Hmac;
    use sha2::Sha256;

    let data = fs::read(encrypted_path)?;
    if data.len() < 16 + 12 {
        return Err(anyhow!("私钥文件格式错误"));
    }
    let (salt, rest) = data.split_at(16);
    let (nonce, ciphertext) = rest.split_at(12);

    // 派生密钥
    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, 100_000, &mut key)
        .map_err(|e| anyhow!("PBKDF2 失败: {:?}", e))?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow!("解密失败，密码错误或数据损坏"))?;

    key.zeroize();
    Ok(plain)
}

// ---------- 签名 ----------
/// 使用 Ed25519 私钥对消息签名
pub fn sign_message(private_key_bytes: &[u8], message: &[u8]) -> Result<Vec<u8>> {
        let keypair = Ed25519KeyPair::from_seed_and_public_key(
        private_key_bytes
            .try_into()
            .map_err(|_| anyhow!("私钥长度错误"))?,
        &private_key_bytes[32..],
    )
    .map_err(|e| anyhow!("密钥对生成失败: {:?}", e))?;
    Ok(keypair.sign(message).as_ref().to_vec())
}

/// 验证 Ed25519 签名（使用公钥字节）
pub fn verify_signature(
    public_key_bytes: &[u8],
    message: &[u8],
    signature: &[u8],
) -> bool {
    let peer_public_key =
        signature::UnparsedPublicKey::new(&signature::ED25519, public_key_bytes);
    peer_public_key.verify(message, signature).is_ok()
}

// ---------- SHA3-256 Base64 ----------
pub fn sha3_256_base64(data: &[u8]) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    STANDARD.encode(hash)
}

// ---------- 安全私钥容器 ----------
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecurePrivateKey {
    bytes: Vec<u8>,
}

impl SecurePrivateKey {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
    // 允许在闭包中临时使用
    pub fn use_key<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.bytes)
    }
}