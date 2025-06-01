CREATE TABLE IF NOT EXISTS user_configs (
    user_id TEXT PRIMARY KEY,
    password_hash TEXT NOT NULL,
    salt1 TEXT NOT NULL,
    salt2 TEXT NOT NULL
); 