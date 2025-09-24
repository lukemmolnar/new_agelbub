-- Users table to track Discord users and their keys
CREATE TABLE users (
    discord_id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    public_key TEXT NOT NULL,
    encrypted_private_key TEXT NOT NULL,
    nonce INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Transaction ledger - immutable append-only log
CREATE TABLE transactions (
    id TEXT PRIMARY KEY,
    from_user TEXT NOT NULL,
    to_user TEXT NOT NULL,
    amount INTEGER NOT NULL,  -- Store in smallest units to avoid floating point
    transaction_type TEXT NOT NULL DEFAULT 'transfer', -- 'transfer', 'mint', 'burn'
    message TEXT,
    nonce INTEGER NOT NULL,
    signature TEXT NOT NULL,
    timestamp_unix INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (from_user) REFERENCES users(discord_id),
    FOREIGN KEY (to_user) REFERENCES users(discord_id)
);

-- Balance cache for performance (calculated from transactions)
CREATE TABLE balances (
    discord_id TEXT PRIMARY KEY,
    balance INTEGER NOT NULL DEFAULT 0,
    last_updated DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (discord_id) REFERENCES users(discord_id)
);

-- Indexes for performance
CREATE INDEX idx_transactions_from_user ON transactions(from_user);
CREATE INDEX idx_transactions_to_user ON transactions(to_user);
CREATE INDEX idx_transactions_timestamp ON transactions(timestamp_unix);
CREATE INDEX idx_users_username ON users(username);

-- Triggers to update the updated_at timestamp
CREATE TRIGGER update_users_timestamp 
    AFTER UPDATE ON users
    BEGIN
        UPDATE users SET updated_at = CURRENT_TIMESTAMP WHERE discord_id = NEW.discord_id;
    END;

CREATE TRIGGER update_balances_timestamp 
    AFTER UPDATE ON balances
    BEGIN
        UPDATE balances SET last_updated = CURRENT_TIMESTAMP WHERE discord_id = NEW.discord_id;
    END;
