CREATE TABLE IF NOT EXISTS user_configs (
    user_id TEXT PRIMARY KEY,
    password_hash TEXT NOT NULL,
    salt1 TEXT NOT NULL,
    salt2 TEXT NOT NULL,
    nonce1 TEXT NOT NULL,
    nonce2 TEXT NOT NULL,
    double_encrypted_private_key TEXT NOT NULL
); 