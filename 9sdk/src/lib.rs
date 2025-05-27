// Cargo.toml dependencies:
//
// [dependencies]
// rand = "0.8"
// argon2 = "0.5"
// password-hash = "0.5"
// aes-gcm = "0.10"

use rand_core::OsRng;
use rand_core::RngCore;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use aes_gcm::{Aes256Gcm, aead::{Aead, KeyInit, Nonce, consts::U12}};
use rand::{thread_rng, Rng};
use aes_gcm::aead::AeadCore;

/// Hash the user's password (for verification only)
fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let argon2 = Argon2::default();

    // Generate password hash
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    password_hash
}

/// Verify a user's password against the stored hash
fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parsed_hash = PasswordHash::new(stored_hash).unwrap();
    let argon2 = Argon2::default();

    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Derive a 256-bit AES key from the password and salt
fn derive_encryption_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32]; // AES-256 key
    let argon2 = Argon2::default();

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .unwrap();

    key
}

/// Encrypt the private key using AES-GCM
fn encrypt_private_key(aes_key: &[u8], plaintext: &[u8]) -> (Vec<u8>, [u8; 12]) {
    let cipher = Aes256Gcm::new_from_slice(aes_key).unwrap();
    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::<Aes256Gcm>::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .unwrap();

    (ciphertext, nonce_bytes)
}

/// Decrypt the private key
fn decrypt_private_key(aes_key: &[u8], ciphertext: &[u8], nonce_bytes: &[u8; 12]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(aes_key).unwrap();
    let nonce = Nonce::<Aes256Gcm>::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .unwrap();

    plaintext
}

pub fn password_handler() -> String {
    // 🌟 1) User sets a password and provides a private key to store
    let user_password = "my_secure_password";
    let user_private_key = b"super_secret_private_key_data";

    // 🌟 2) Hash the password for authentication
    let password_hash = hash_password(user_password);

    // 🌟 3) Generate a salt for key derivation (separate from password hash)
    let mut salt = [0u8; 16];
    thread_rng().fill(&mut salt);

    // 🌟 4) Derive an AES encryption key from password+salt
    let encryption_key = derive_encryption_key(user_password, &salt);

    // 🌟 5) Encrypt the private key
    let (encrypted_key, nonce) = encrypt_private_key(&encryption_key, user_private_key);

    // 🔒 Now store in DB:
    // - password_hash (for authentication)
    // - salt
    // - nonce
    // - encrypted_key

    // 🪪 Let's simulate a "database" here:
    let stored_password_hash = password_hash;
    let stored_salt = salt;
    let stored_nonce = nonce;
    let stored_encrypted_key = encrypted_key;

    // 🌟 6) Later, user tries to access their private key
    let user_password_attempt = "my_secure_password";

    // 🌟 7) Verify the password
    let is_password_correct = verify_password(user_password_attempt, &stored_password_hash);
    if !is_password_correct {
        println!("❌ Incorrect password! Aborting.");
        return String::new();
    }
    println!("✅ Password verified.");

    // 🌟 8) Re-derive the AES key using the stored salt
    let derived_key = derive_encryption_key(user_password_attempt, &stored_salt);

    // 🌟 9) Decrypt the private key
    let decrypted_private_key = decrypt_private_key(&derived_key, &stored_encrypted_key, &stored_nonce);

    String::from_utf8(decrypted_private_key).unwrap()
}



//pub fn password_handler() -> &'static str {
//    "Lets do that password thang"
//}