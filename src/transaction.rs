//! 交易数据结构及序列化

use serde::{Deserialize, Serialize};
use crate::crypto::sha3_256_base64;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub from: String,   // "游戏ID <邮箱>"
    pub to: String,
    pub amount: f64,
    pub timestamp: String,
    pub hash: String,
    pub from_signature: String,  // base64
    pub to_signature: Option<String>,  // 收款方确认后填写
    pub status: String,  // "pending", "completed"
}

impl Transaction {
    /// 计算交易哈希（不含签名）
    pub fn calculate_hash(&self) -> String {
        let content = format!(
            "{}{}{}{}{}{}",
            self.id, self.type_, self.from, self.to, self.amount, self.timestamp
        );
        sha3_256_base64(content.as_bytes())
    }

    /// 验证付款方签名
    pub fn verify_from_signature(&self, from_pubkey: &[u8]) -> bool {
        let msg = format!("{}:{}", self.id, self.amount);
        let sig = STANDARD.decode(&self.from_signature).unwrap_or_default();
        crate::crypto::verify_signature(from_pubkey, msg.as_bytes(), &sig)
    }

    /// 验证收款方签名（如果有）
    pub fn verify_to_signature(&self, to_pubkey: &[u8]) -> bool {
        if let Some(sig_b64) = &self.to_signature {
            let msg = format!("ACK:{}", self.id);
            let sig = STANDARD.decode(sig_b64).unwrap_or_default();
            crate::crypto::verify_signature(to_pubkey, msg.as_bytes(), &sig)
        } else {
            false
        }
    }
}

/// 交换用的交易包：包含交易JSON和发送方公钥文件名
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionPackage {
    pub transaction: Transaction,
    pub sender_pubkey_filename: String, // 建议如 "sender.asc"
}