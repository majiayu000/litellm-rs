//! Backup code generation and hashing

use rand::Rng;
use sha2::{Digest, Sha256};

/// Generate a secure backup code
pub fn generate_backup_code() -> String {
    let mut rng = rand::rng();
    let code: String = (0..8)
        .map(|_| rng.random_range(0..10u32).to_string())
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.join(""))
        .collect::<Vec<_>>()
        .join("-");
    code
}

/// Generate multiple backup codes
pub fn generate_backup_codes(count: usize) -> Vec<String> {
    (0..count).map(|_| generate_backup_code()).collect()
}

/// Hash backup code for storage
pub fn hash_backup_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hasher.update(b"backup_code_salt"); // Simple salt
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== generate_backup_code Tests ====================

    #[test]
    fn test_generate_backup_code_format() {
        let code = generate_backup_code();
        // Format: XXXX-XXXX
        assert_eq!(code.len(), 9);
        assert_eq!(&code[4..5], "-");
    }

    #[test]
    fn test_generate_backup_code_digits_only() {
        let code = generate_backup_code();
        let parts: Vec<&str> = code.split('-').collect();
        assert_eq!(parts.len(), 2);
        for part in parts {
            assert!(part.chars().all(|c| c.is_ascii_digit()));
            assert_eq!(part.len(), 4);
        }
    }

    #[test]
    fn test_generate_backup_code_uniqueness() {
        let code1 = generate_backup_code();
        let code2 = generate_backup_code();
        // Very unlikely to be the same (1 in 100 million chance)
        assert_ne!(code1, code2);
    }

    // ==================== generate_backup_codes Tests ====================

    #[test]
    fn test_generate_backup_codes_count() {
        let codes = generate_backup_codes(10);
        assert_eq!(codes.len(), 10);
    }

    #[test]
    fn test_generate_backup_codes_zero() {
        let codes = generate_backup_codes(0);
        assert!(codes.is_empty());
    }

    #[test]
    fn test_generate_backup_codes_uniqueness() {
        let codes = generate_backup_codes(10);
        let unique_codes: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique_codes.len(), codes.len());
    }

    #[test]
    fn test_generate_backup_codes_format() {
        let codes = generate_backup_codes(5);
        for code in codes {
            assert_eq!(code.len(), 9);
            assert_eq!(&code[4..5], "-");
        }
    }

    // ==================== hash_backup_code Tests ====================

    #[test]
    fn test_hash_backup_code_length() {
        let hash = hash_backup_code("1234-5678");
        assert_eq!(hash.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn test_hash_backup_code_consistency() {
        let code = "1234-5678";
        let hash1 = hash_backup_code(code);
        let hash2 = hash_backup_code(code);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_backup_code_different_codes() {
        let hash1 = hash_backup_code("1234-5678");
        let hash2 = hash_backup_code("8765-4321");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_backup_code_hex_format() {
        let hash = hash_backup_code("0000-0000");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_backup_code_empty() {
        let hash = hash_backup_code("");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_backup_code_with_salt() {
        // Verify that the salt affects the hash
        let hash_with_salt = hash_backup_code("1234-5678");

        // Calculate hash without salt for comparison
        let mut hasher = Sha256::new();
        hasher.update("1234-5678".as_bytes());
        let hash_without_salt = hex::encode(hasher.finalize());

        assert_ne!(hash_with_salt, hash_without_salt);
    }
}
