//! 客户端加密：Argon2 派生密钥 + ChaCha20-Poly1305 + 确定性 nonce。
//!
//! 设计要点：
//! - `nonce = HMAC-SHA256(masterKey, nonceId)[..12]`，因此**同一明文（同一 nonceId）
//!   在任何设备上得到同一密文**，PUT 幂等、不产生对象级冲突。
//! - `nonceId` 对 blob 用内容哈希，对设备清单用 `deviceId`（读取方已知）。
//! - blob 结构：`magic(4) | version(1) | nonce(12) | ciphertext+tag`。
//! - 口令丢失 = 数据不可恢复。

use crate::error::{AppError, AppResult};
use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

const MAGIC: &[u8; 4] = b"SKB1";
const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const HEADER_LEN: usize = 4 + 1 + NONCE_LEN;

/// Argon2 盐长度（字节）。盐不是秘密，需在设备间共享（存 bucket `_format.json`）。
pub const SALT_LEN: usize = 16;

/// 持有派生自口令的主密钥。Drop 时 `Zeroizing` 负责擦除。
#[derive(Clone)]
pub struct Crypto {
    key: Zeroizing<[u8; KEY_LEN]>,
}

impl Crypto {
    /// 由口令 + 共享盐派生主密钥。盐必须 >= 8 字节。
    pub fn derive_key(password: &str, salt: &[u8]) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
        if salt.len() < 8 {
            return Err(AppError::Message("加密盐长度不足（至少 8 字节）".to_string()));
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        Argon2::default()
            .hash_password_into(password.as_bytes(), salt, key.as_mut())
            .map_err(|error| AppError::Message(format!("密钥派生失败: {error}")))?;
        Ok(key)
    }

    pub fn from_password(password: &str, salt: &[u8]) -> AppResult<Self> {
        Ok(Self {
            key: Self::derive_key(password, salt)?,
        })
    }

    /// 由已派生的主密钥构造（用于测试 / 缓存）。
    pub fn from_key(key: [u8; KEY_LEN]) -> Self {
        Self {
            key: Zeroizing::new(key),
        }
    }

    pub fn generate_salt() -> [u8; SALT_LEN] {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        salt
    }

    fn nonce_for(&self, nonce_id: &str) -> Nonce {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(self.key.as_ref())
            .expect("HMAC 接受任意长度密钥");
        mac.update(nonce_id.as_bytes());
        let tag = mac.finalize().into_bytes();
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&tag[..NONCE_LEN]);
        Nonce::from(nonce)
    }

    /// 加密。`nonce_id` 必须是两端都能确定的稳定标识（内容哈希 / deviceId）。
    pub fn encrypt(&self, plaintext: &[u8], nonce_id: &str) -> AppResult<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| AppError::Message("密钥长度无效".to_string()))?;
        let nonce = self.nonce_for(nonce_id);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: nonce_id.as_bytes(),
                },
            )
            .map_err(|_| AppError::Message("加密失败".to_string()))?;

        let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// 解密。篡改 / 口令错误 / `nonce_id` 不符都会失败。
    pub fn decrypt(&self, ciphertext: &[u8], nonce_id: &str) -> AppResult<Vec<u8>> {
        if ciphertext.len() < HEADER_LEN {
            return Err(AppError::Message("密文长度不足".to_string()));
        }
        if &ciphertext[0..4] != MAGIC {
            return Err(AppError::Message("密文格式错误（magic 不符）".to_string()));
        }
        if ciphertext[4] != VERSION {
            return Err(AppError::Message(format!(
                "不支持的密文版本: {}",
                ciphertext[4]
            )));
        }
        let nonce = &ciphertext[5..HEADER_LEN];
        let body = &ciphertext[HEADER_LEN..];
        let cipher = ChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| AppError::Message("密钥长度无效".to_string()))?;
        cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: body,
                    aad: nonce_id.as_bytes(),
                },
            )
            .map_err(|_| AppError::Message("解密失败（口令错误或数据损坏）".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crypto() -> Crypto {
        Crypto::from_password("correct horse battery staple", b"0123456789abcdef").unwrap()
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let crypto = crypto();
        let plaintext = b"hello skill";
        let ciphertext = crypto.encrypt(plaintext, "abc123").unwrap();
        assert_ne!(&ciphertext[HEADER_LEN..], plaintext.as_slice());
        assert_eq!(crypto.decrypt(&ciphertext, "abc123").unwrap(), plaintext);
    }

    #[test]
    fn wrong_password_fails() {
        let ciphertext = crypto().encrypt(b"secret", "id").unwrap();
        let other = Crypto::from_password("wrong", b"0123456789abcdef").unwrap();
        assert!(other.decrypt(&ciphertext, "id").is_err());
    }

    #[test]
    fn deterministic_nonce_same_plaintext_same_ciphertext() {
        let crypto = crypto();
        let a = crypto.encrypt(b"same", "content-hash").unwrap();
        let b = crypto.encrypt(b"same", "content-hash").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_nonce_id_differs() {
        let crypto = crypto();
        let a = crypto.encrypt(b"same", "id-a").unwrap();
        let b = crypto.encrypt(b"same", "id-b").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let crypto = crypto();
        let mut ciphertext = crypto.encrypt(b"payload", "id").unwrap();
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 0xFF;
        assert!(crypto.decrypt(&ciphertext, "id").is_err());
    }

    #[test]
    fn wrong_nonce_id_rejected() {
        let crypto = crypto();
        let ciphertext = crypto.encrypt(b"payload", "id-a").unwrap();
        assert!(crypto.decrypt(&ciphertext, "id-b").is_err());
    }

    #[test]
    fn salt_round_trips_between_devices() {
        // 两台设备共享盐 + 口令 → 密钥一致 → 可互相解密
        let salt = Crypto::generate_salt();
        let a = Crypto::from_password("pw", &salt).unwrap();
        let b = Crypto::from_password("pw", &salt).unwrap();
        let ciphertext = a.encrypt(b"shared", "blob-hash").unwrap();
        assert_eq!(b.decrypt(&ciphertext, "blob-hash").unwrap(), b"shared");
    }

    #[test]
    fn short_salt_rejected() {
        assert!(Crypto::from_password("pw", b"short").is_err());
    }
}
