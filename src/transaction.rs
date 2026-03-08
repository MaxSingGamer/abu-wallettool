 交易数据结构及序列化

use serde::{Deserialize, Serialize};
use crate::crypto::sha3_256_base64;

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

}