use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub discord_id: String,
    pub username: String,
    pub public_key: String,
    pub encrypted_private_key: String,
    pub nonce: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub from_user: String,
    pub to_user: String,
    pub amount: i64,
    pub transaction_type: String,
    pub message: Option<String>,
    pub nonce: i64,
    pub signature: String,
    pub timestamp_unix: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Balance {
    pub discord_id: String,
    pub balance: i64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        // Ensure the database directory exists
        if let Some(parent) = Path::new(database_url).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| sqlx::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        }

        let pool = SqlitePool::connect(database_url).await?;
        
        // Create tables if they don't exist
        Self::create_tables(&pool).await?;
        
        info!("Database connected and migrations applied");
        
        Ok(Database { pool })
    }

    async fn create_tables(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        // Create users table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                discord_id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                public_key TEXT NOT NULL,
                encrypted_private_key TEXT NOT NULL,
                nonce INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(pool)
        .await?;

        // Create transactions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                from_user TEXT NOT NULL,
                to_user TEXT NOT NULL,
                amount INTEGER NOT NULL,
                transaction_type TEXT NOT NULL DEFAULT 'transfer',
                message TEXT,
                nonce INTEGER NOT NULL,
                signature TEXT NOT NULL,
                timestamp_unix INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(pool)
        .await?;

        // Create balances table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS balances (
                discord_id TEXT PRIMARY KEY,
                balance INTEGER NOT NULL DEFAULT 0,
                last_updated DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(pool)
        .await?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_transactions_from_user ON transactions(from_user)")
            .execute(pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_transactions_to_user ON transactions(to_user)")
            .execute(pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_transactions_timestamp ON transactions(timestamp_unix)")
            .execute(pool)
            .await?;

        info!("Database tables created successfully");
        Ok(())
    }

    // User management
    pub async fn create_user(&self, user: &User) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO users (discord_id, username, public_key, encrypted_private_key, nonce) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&user.discord_id)
        .bind(&user.username)
        .bind(&user.public_key)
        .bind(&user.encrypted_private_key)
        .bind(user.nonce)
        .execute(&self.pool)
        .await?;

        // Initialize balance
        sqlx::query("INSERT INTO balances (discord_id, balance) VALUES (?, 0)")
            .bind(&user.discord_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_user(&self, discord_id: &str) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT discord_id, username, public_key, encrypted_private_key, nonce, created_at, updated_at FROM users WHERE discord_id = ?"
        )
        .bind(discord_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(User {
                discord_id: row.get("discord_id"),
                username: row.get("username"),
                public_key: row.get("public_key"),
                encrypted_private_key: row.get("encrypted_private_key"),
                nonce: row.get("nonce"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update_user_nonce(&self, discord_id: &str, nonce: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET nonce = ? WHERE discord_id = ?")
            .bind(nonce)
            .bind(discord_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // Transaction management
    pub async fn add_transaction(&self, transaction: &Transaction) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO transactions 
            (id, from_user, to_user, amount, transaction_type, message, nonce, signature, timestamp_unix)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&transaction.id)
        .bind(&transaction.from_user)
        .bind(&transaction.to_user)
        .bind(transaction.amount)
        .bind(&transaction.transaction_type)
        .bind(&transaction.message)
        .bind(transaction.nonce)
        .bind(&transaction.signature)
        .bind(transaction.timestamp_unix)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_user_transactions(&self, discord_id: &str) -> Result<Vec<Transaction>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, from_user, to_user, amount, transaction_type, message, nonce, signature, timestamp_unix, created_at
            FROM transactions 
            WHERE from_user = ? OR to_user = ? 
            ORDER BY timestamp_unix DESC
            "#
        )
        .bind(discord_id)
        .bind(discord_id)
        .fetch_all(&self.pool)
        .await?;

        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(Transaction {
                id: row.get("id"),
                from_user: row.get("from_user"),
                to_user: row.get("to_user"),
                amount: row.get("amount"),
                transaction_type: row.get("transaction_type"),
                message: row.get("message"),
                nonce: row.get("nonce"),
                signature: row.get("signature"),
                timestamp_unix: row.get("timestamp_unix"),
                created_at: row.get("created_at"),
            });
        }

        Ok(transactions)
    }

    pub async fn get_all_transactions(&self) -> Result<Vec<Transaction>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, from_user, to_user, amount, transaction_type, message, nonce, signature, timestamp_unix, created_at FROM transactions ORDER BY timestamp_unix ASC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(Transaction {
                id: row.get("id"),
                from_user: row.get("from_user"),
                to_user: row.get("to_user"),
                amount: row.get("amount"),
                transaction_type: row.get("transaction_type"),
                message: row.get("message"),
                nonce: row.get("nonce"),
                signature: row.get("signature"),
                timestamp_unix: row.get("timestamp_unix"),
                created_at: row.get("created_at"),
            });
        }

        Ok(transactions)
    }

    // Balance management
    pub async fn get_balance(&self, discord_id: &str) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT balance FROM balances WHERE discord_id = ?")
            .bind(discord_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("balance")).unwrap_or(0))
    }

    pub async fn update_balance(&self, discord_id: &str, new_balance: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO balances (discord_id, balance) 
            VALUES (?, ?)
            ON CONFLICT(discord_id) 
            DO UPDATE SET balance = ?, last_updated = CURRENT_TIMESTAMP
            "#
        )
        .bind(discord_id)
        .bind(new_balance)
        .bind(new_balance)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Utility functions
    pub async fn calculate_balance_from_transactions(&self, discord_id: &str) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                COALESCE(SUM(CASE WHEN to_user = ? THEN amount ELSE 0 END), 0) -
                COALESCE(SUM(CASE WHEN from_user = ? THEN amount ELSE 0 END), 0) as balance
            FROM transactions
            WHERE from_user = ? OR to_user = ?
            "#
        )
        .bind(discord_id)
        .bind(discord_id)
        .bind(discord_id)
        .bind(discord_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("balance"))
    }

    pub async fn verify_and_update_balances(&self) -> Result<(), sqlx::Error> {
        info!("Verifying and updating all balances from transaction ledger");
        
        let rows = sqlx::query("SELECT discord_id FROM users")
            .fetch_all(&self.pool)
            .await?;

        for row in rows {
            let discord_id: String = row.get("discord_id");
            let calculated_balance = self.calculate_balance_from_transactions(&discord_id).await?;
            self.update_balance(&discord_id, calculated_balance).await?;
        }

        info!("Balance verification complete");
        Ok(())
    }

    // Get all users with their balances for leaderboard
    pub async fn get_all_users_with_balances(&self, limit: Option<u32>) -> Result<Vec<(String, i64)>, sqlx::Error> {
        let query = match limit {
            Some(limit_val) => format!(
                r#"
                SELECT u.username, COALESCE(b.balance, 0) as balance
                FROM users u
                LEFT JOIN balances b ON u.discord_id = b.discord_id
                ORDER BY COALESCE(b.balance, 0) DESC
                LIMIT {}
                "#,
                limit_val
            ),
            None => r#"
                SELECT u.username, COALESCE(b.balance, 0) as balance
                FROM users u
                LEFT JOIN balances b ON u.discord_id = b.discord_id
                ORDER BY COALESCE(b.balance, 0) DESC
                "#.to_string(),
        };

        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await?;

        let mut users_with_balances = Vec::new();
        for row in rows {
            let username: String = row.get("username");
            let balance: i64 = row.get("balance");
            users_with_balances.push((username, balance));
        }

        Ok(users_with_balances)
    }
}
