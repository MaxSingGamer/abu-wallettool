 个人账本 SQLite 数据库

use rusqlite::{params, Connection};  // 去掉 SqlResult
use anyhow::Result;  // 去掉 anyhow
use std::path::Path;

/// 初始化数据库（若不存在则创建）
pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    // 创建交易表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transactions (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            from_addr TEXT NOT NULL,
            to_addr TEXT NOT NULL,
            amount REAL NOT NULL,
            timestamp TEXT NOT NULL,
            hash TEXT NOT NULL,
            from_signature TEXT,
            to_signature TEXT,
            status TEXT DEFAULT 'pending'
        )",
        [],
    )?;
    // 创建余额表（方便快速查询）
    conn.execute(
        "CREATE TABLE IF NOT EXISTS balances (
            account TEXT PRIMARY KEY,
            balance REAL NOT NULL
        )",
        [],
    )?;
    Ok(conn)
}

/// 插入交易，更新余额
pub fn insert_transaction(
    conn: &Connection,
    tx: &crate::transaction::Transaction,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO transactions 
         (id, type, from_addr, to_addr, amount, timestamp, hash, from_signature, to_signature, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            tx.id,
            tx.type_,
            tx.from,
            tx.to,
            tx.amount,
            tx.timestamp,
            tx.hash,
            tx.from_signature,
            tx.to_signature,
            tx.status,
        ],
    )?;
    // 更新付款方余额
    conn.execute(
        "UPDATE balances SET balance = balance - ?1 WHERE account = ?2",
        params![tx.amount, tx.from],
    )?;
    // 更新收款方余额
    conn.execute(
        "UPDATE balances SET balance = balance + ?1 WHERE account = ?2",
        params![tx.amount, tx.to],
    )?;
    Ok(())
}

/// 获取账户余额
pub fn get_balance(conn: &Connection, account: &str) -> Result<f64> {
    let balance: f64 = conn
        .query_row(
            "SELECT balance FROM balances WHERE account = ?1",
            params![account],
            |row| row.get(0),
        )
        .unwrap_or(0.0);
    Ok(balance)
}

/// 获取所有交易（用于导出证据）
pub fn get_all_transactions(conn: &Connection) -> Result<Vec<crate::transaction::Transaction>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, from_addr, to_addr, amount, timestamp, hash, from_signature, to_signature, status FROM transactions",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::transaction::Transaction {
            id: row.get(0)?,
            type_: row.get(1)?,
            from: row.get(2)?,
            to: row.get(3)?,
            amount: row.get(4)?,
            timestamp: row.get(5)?,
            hash: row.get(6)?,
            from_signature: row.get(7)?,
            to_signature: row.get(8)?,
            status: row.get(9)?,
        })
    })?;
    let mut txs = Vec::new();
    for row in rows {
        txs.push(row?);
    }
    Ok(txs)
}