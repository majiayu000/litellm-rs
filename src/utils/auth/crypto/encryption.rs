//! Encryption utilities including AES-GCM and hashing with salt

use crate::utils::error::gateway_error::{GatewayError, Result};
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose};
use rand::{Rng, RngCore};
use sha2::{Digest, Sha256};

/// AES-256-GCM nonce size (96 bits / 12 bytes as recommended by NIST)
const AES_GCM_NONCE_SIZE: usize = 12;

/// Derive a 256-bit key from arbitrary-length input using SHA-256
fn derive_key(key: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(key);
    hasher.finalize().into()
}

/// Encrypt data using AES-256-GCM with authenticated encryption.
///
/// The output format is: base64(nonce || ciphertext || tag)
/// - nonce: 12 bytes (randomly generated)
/// - ciphertext: variable length (same as plaintext)
/// - tag: 16 bytes (authentication tag)
///
/// # Security
/// - Uses cryptographically secure random nonce for each encryption
/// - Provides both confidentiality and integrity protection
/// - Key is derived using SHA-256 if not exactly 32 bytes
pub fn encrypt_data(key: &[u8], data: &str) -> Result<String> {
    // Derive 256-bit key from input
    let derived_key = derive_key(key);
    let cipher_key = Key::<Aes256Gcm>::from_slice(&derived_key);
    let cipher = Aes256Gcm::new(cipher_key);

    // Generate random 96-bit nonce (12 bytes)
    let mut nonce_bytes = [0u8; AES_GCM_NONCE_SIZE];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt the data
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .map_err(|e| GatewayError::Crypto(format!("Encryption failed: {}", e)))?;

    // Prepend nonce to ciphertext for storage
    let mut output = Vec::with_capacity(AES_GCM_NONCE_SIZE + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    // Encode as base64 for safe storage/transmission
    Ok(general_purpose::STANDARD.encode(&output))
}

/// Decrypt data encrypted with AES-256-GCM.
///
/// Expects input format: base64(nonce || ciphertext || tag)
///
/// # Security
/// - Verifies authentication tag before returning plaintext
/// - Returns error if data has been tampered with
pub fn decrypt_data(key: &[u8], encrypted_data: &str) -> Result<String> {
    // Decode base64 encrypted data
    let encrypted_bytes = general_purpose::STANDARD
        .decode(encrypted_data)
        .map_err(|e| GatewayError::Crypto(format!("Failed to decode encrypted data: {}", e)))?;

    // Validate minimum length (nonce + at least 16-byte auth tag)
    if encrypted_bytes.len() < AES_GCM_NONCE_SIZE + 16 {
        return Err(GatewayError::Crypto(
            "Encrypted data too short - possible corruption or tampering".to_string(),
        ));
    }

    // Derive 256-bit key from input
    let derived_key = derive_key(key);
    let cipher_key = Key::<Aes256Gcm>::from_slice(&derived_key);
    let cipher = Aes256Gcm::new(cipher_key);

    // Extract nonce and ciphertext
    let nonce = Nonce::from_slice(&encrypted_bytes[..AES_GCM_NONCE_SIZE]);
    let ciphertext = &encrypted_bytes[AES_GCM_NONCE_SIZE..];

    // Decrypt and verify authentication tag
    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
        GatewayError::Crypto(
            "Decryption failed - data may have been tampered with or wrong key".to_string(),
        )
    })?;

    String::from_utf8(plaintext).map_err(|e| {
        GatewayError::Crypto(format!("Failed to convert decrypted data to string: {}", e))
    })
}

/// Generate a secure random salt
pub fn generate_salt() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.random()).collect();
    general_purpose::STANDARD.encode(&bytes)
}

/// Hash data with salt
pub fn hash_with_salt(data: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hasher.update(salt.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a time-based one-time password (TOTP) secret
pub fn generate_totp_secret() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..20).map(|_| rng.random()).collect();
    general_purpose::STANDARD.encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== encrypt_data / decrypt_data Tests ====================

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = b"my-secret-key-32-bytes-long!!!!!";
        let plaintext = "Hello, World!";

        let encrypted = encrypt_data(key, plaintext).unwrap();
        let decrypted = decrypt_data(key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty_string() {
        let key = b"test-key";
        let plaintext = "";

        let encrypted = encrypt_data(key, plaintext).unwrap();
        let decrypted = decrypt_data(key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_unicode() {
        let key = b"unicode-test-key";
        let plaintext = "你好世界 🌍 Привет мир";

        let encrypted = encrypt_data(key, plaintext).unwrap();
        let decrypted = decrypt_data(key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_long_text() {
        let key = b"long-text-key";
        let plaintext = "x".repeat(10000);

        let encrypted = encrypt_data(key, &plaintext).unwrap();
        let decrypted = decrypt_data(key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_different_output_each_time() {
        let key = b"nonce-test-key";
        let plaintext = "same message";

        let encrypted1 = encrypt_data(key, plaintext).unwrap();
        let encrypted2 = encrypt_data(key, plaintext).unwrap();

        // Due to random nonce, same plaintext should produce different ciphertext
        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key1 = b"correct-key";
        let key2 = b"wrong-key";
        let plaintext = "secret data";

        let encrypted = encrypt_data(key1, plaintext).unwrap();
        let result = decrypt_data(key2, &encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_base64_fails() {
        let key = b"test-key";
        let result = decrypt_data(key, "not-valid-base64!!!");

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short_data_fails() {
        let key = b"test-key";
        // Valid base64 but too short to contain nonce + tag
        let result = decrypt_data(key, "AAAA");

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_tampered_data_fails() {
        let key = b"tamper-test-key";
        let plaintext = "sensitive data";

        let encrypted = encrypt_data(key, plaintext).unwrap();
        let mut encrypted_bytes = general_purpose::STANDARD.decode(&encrypted).unwrap();

        // Tamper with the ciphertext
        if let Some(byte) = encrypted_bytes.get_mut(20) {
            *byte ^= 0xFF;
        }

        let tampered = general_purpose::STANDARD.encode(&encrypted_bytes);
        let result = decrypt_data(key, &tampered);

        assert!(result.is_err());
    }

    // ==================== generate_salt Tests ====================

    #[test]
    fn test_generate_salt_not_empty() {
        let salt = generate_salt();
        assert!(!salt.is_empty());
    }

    #[test]
    fn test_generate_salt_is_base64() {
        let salt = generate_salt();
        let decoded = general_purpose::STANDARD.decode(&salt);
        assert!(decoded.is_ok());
    }

    #[test]
    fn test_generate_salt_unique() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();
        assert_ne!(salt1, salt2);
    }

    #[test]
    fn test_generate_salt_length() {
        let salt = generate_salt();
        let decoded = general_purpose::STANDARD.decode(&salt).unwrap();
        assert_eq!(decoded.len(), 16);
    }

    // ==================== hash_with_salt Tests ====================

    #[test]
    fn test_hash_with_salt_consistency() {
        let data = "password123";
        let salt = "my-salt";

        let hash1 = hash_with_salt(data, salt);
        let hash2 = hash_with_salt(data, salt);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_with_salt_different_data() {
        let salt = "same-salt";

        let hash1 = hash_with_salt("data1", salt);
        let hash2 = hash_with_salt("data2", salt);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_with_salt_different_salt() {
        let data = "same-data";

        let hash1 = hash_with_salt(data, "salt1");
        let hash2 = hash_with_salt(data, "salt2");

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_with_salt_hex_format() {
        let hash = hash_with_salt("test", "salt");
        assert_eq!(hash.len(), 64); // SHA256 = 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ==================== generate_totp_secret Tests ====================

    #[test]
    fn test_generate_totp_secret_not_empty() {
        let secret = generate_totp_secret();
        assert!(!secret.is_empty());
    }

    #[test]
    fn test_generate_totp_secret_is_base64() {
        let secret = generate_totp_secret();
        let decoded = general_purpose::STANDARD.decode(&secret);
        assert!(decoded.is_ok());
    }

    #[test]
    fn test_generate_totp_secret_length() {
        let secret = generate_totp_secret();
        let decoded = general_purpose::STANDARD.decode(&secret).unwrap();
        assert_eq!(decoded.len(), 20);
    }

    #[test]
    fn test_generate_totp_secret_unique() {
        let secret1 = generate_totp_secret();
        let secret2 = generate_totp_secret();
        assert_ne!(secret1, secret2);
    }
}
