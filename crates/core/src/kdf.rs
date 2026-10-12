//! Argon2id key derivation for the encrypted workspace. The derived bytes
//! are imported straight into WebCrypto as a non-extractable AES-GCM key.

use crate::error::{Error, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KdfOptions {
    pub passphrase: String,
    /// Memory cost in KiB.
    pub memory_kib: u32,
    pub iterations: u32,
}

impl Default for KdfOptions {
    fn default() -> Self {
        // OWASP's Argon2id baseline: 64 MiB, 3 passes, 1 lane.
        KdfOptions {
            passphrase: String::new(),
            memory_kib: 64 * 1024,
            iterations: 3,
        }
    }
}

pub fn derive_key(salt: &[u8], o: &KdfOptions) -> Result<[u8; 32]> {
    if o.passphrase.is_empty() {
        return Err(Error::option("the passphrase is empty"));
    }
    if salt.len() < 16 {
        return Err(Error::option("the salt must be at least 16 bytes"));
    }
    if !(8 * 1024..=1024 * 1024).contains(&o.memory_kib) || !(1..=20).contains(&o.iterations) {
        return Err(Error::option(
            "Argon2 cost is outside 8 MiB-1 GiB, 1-20 passes",
        ));
    }
    let params = Params::new(o.memory_kib, o.iterations, 1, Some(32))
        .map_err(|e| Error::option(e.to_string()))?;
    let mut key = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(o.passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| Error::option(e.to_string()))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_salted() {
        let o = KdfOptions {
            passphrase: "correct horse".into(),
            memory_kib: 8192,
            iterations: 1,
        };
        let a = derive_key(&[1; 16], &o).unwrap();
        assert_eq!(a, derive_key(&[1; 16], &o).unwrap());
        assert_ne!(a, derive_key(&[2; 16], &o).unwrap());
        let other = KdfOptions {
            passphrase: "wrong horse".into(),
            ..o
        };
        assert_ne!(a, derive_key(&[1; 16], &other).unwrap());
        assert!(
            derive_key(
                &[1; 8],
                &KdfOptions {
                    passphrase: "x".into(),
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
