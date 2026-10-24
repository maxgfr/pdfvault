//! AES-256 (PDF 2.0, revision 6) protection, unlocking and permissions.

use crate::error::{Error, Result};
use crate::pdf;
use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use lopdf::{EncryptionState, EncryptionVersion, Object, Permissions, StringFormat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PermissionSet {
    pub print: bool,
    pub print_high_quality: bool,
    pub modify: bool,
    pub copy: bool,
    pub annotate: bool,
    pub fill_forms: bool,
    pub assemble: bool,
}

impl Default for PermissionSet {
    fn default() -> Self {
        PermissionSet {
            print: true,
            print_high_quality: true,
            modify: true,
            copy: true,
            annotate: true,
            fill_forms: true,
            assemble: true,
        }
    }
}

impl PermissionSet {
    fn flags(&self) -> Permissions {
        let mut p = Permissions::COPYABLE_FOR_ACCESSIBILITY;
        let pairs = [
            (self.print, Permissions::PRINTABLE),
            (
                self.print_high_quality,
                Permissions::PRINTABLE_IN_HIGH_QUALITY,
            ),
            (self.modify, Permissions::MODIFIABLE),
            (self.copy, Permissions::COPYABLE),
            (self.annotate, Permissions::ANNOTABLE),
            (self.fill_forms, Permissions::FILLABLE),
            (self.assemble, Permissions::ASSEMBLABLE),
        ];
        for (on, flag) in pairs {
            if on {
                p |= flag;
            }
        }
        p
    }

    pub fn from_bits(p: i64) -> Self {
        let has = |f: Permissions| (p as u64) & f.bits() != 0;
        PermissionSet {
            print: has(Permissions::PRINTABLE),
            print_high_quality: has(Permissions::PRINTABLE_IN_HIGH_QUALITY),
            modify: has(Permissions::MODIFIABLE),
            copy: has(Permissions::COPYABLE),
            annotate: has(Permissions::ANNOTABLE),
            fill_forms: has(Permissions::FILLABLE),
            assemble: has(Permissions::ASSEMBLABLE),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EncryptOptions {
    /// Needed to open the file. Empty means anyone can open it, and only the
    /// permissions apply.
    pub user_password: String,
    /// Needed to change permissions. Random when not given.
    pub owner_password: Option<String>,
    pub permissions: PermissionSet,
    /// Password of the input, if it is already protected.
    pub password: Option<String>,
}

fn random_password() -> Result<String> {
    let mut b = [0u8; 24];
    getrandom::fill(&mut b).map_err(|e| Error::Unsupported(format!("no random source: {e}")))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

pub fn encrypt(input: &[u8], o: &EncryptOptions) -> Result<Vec<u8>> {
    let owner = match &o.owner_password {
        Some(p) if !p.is_empty() => p.clone(),
        _ => random_password()?,
    };
    if o.user_password.is_empty() && o.owner_password.as_deref().unwrap_or("").is_empty() {
        return Err(Error::option(
            "set a password to open the file, an owner password, or both",
        ));
    }
    if o.user_password.len() > 127 || owner.len() > 127 {
        return Err(Error::option("passwords are limited to 127 bytes"));
    }
    let mut doc = pdf::load(input, o.password.as_deref())?;
    doc.prune_objects();
    doc.renumber_objects();
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|e| Error::Unsupported(format!("no random source: {e}")))?;
    let filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
    let version = EncryptionVersion::V5 {
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
        file_encryption_key: &key,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: &owner,
        user_password: &o.user_password,
        permissions: o.permissions.flags(),
    };
    let state = EncryptionState::try_from(version).map_err(|e| Error::InvalidPdf(e.to_string()))?;
    // Compress before encrypting: ciphertext does not compress.
    doc.compress();
    pdf::fix_lengths(&mut doc);
    doc.encrypt(&state)?;
    // lopdf leaves /Length null for revision 6 and writes no /ID; both are required.
    if let Ok(id) = doc.trailer.get(b"Encrypt").and_then(Object::as_reference)
        && let Ok(d) = doc.get_dictionary_mut(id)
    {
        d.set("Length", 256);
    }
    let mut file_id = [0u8; 16];
    getrandom::fill(&mut file_id)
        .map_err(|e| Error::Unsupported(format!("no random source: {e}")))?;
    let id = Object::String(file_id.to_vec(), StringFormat::Hexadecimal);
    doc.trailer.set("ID", vec![id.clone(), id]);
    doc.version = "2.0".into();
    pdf::save_plain(&mut doc)
}

/// Remove protection, given the user or owner password.
pub fn decrypt(input: &[u8], password: &str) -> Result<Vec<u8>> {
    let mut doc = pdf::load(input, Some(password))?;
    pdf::save(&mut doc)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityInfo {
    pub encrypted: bool,
    pub needs_password: bool,
    pub revision: Option<i64>,
    pub algorithm: Option<String>,
    pub permissions: Option<PermissionSet>,
}

/// Describe a file's protection without needing its password. The /Encrypt
/// dictionary is never inside an object stream, so it is read from the raw bytes.
pub fn inspect(input: &[u8]) -> Result<SecurityInfo> {
    let doc = lopdf::Document::load_mem(input)?;
    // lopdf drops /Encrypt from the trailer once the empty password opens the file.
    let id = match doc.trailer.get(b"Encrypt") {
        Ok(Object::Reference(id)) => Some(*id),
        _ if doc.encryption_state.is_some() => encrypt_ref(input),
        _ => None,
    };
    let Some(id) = id else {
        return Ok(SecurityInfo {
            encrypted: false,
            needs_password: false,
            revision: None,
            algorithm: None,
            permissions: None,
        });
    };
    // Parsed, not scanned: its /O, /U, /OE, /UE and /Perms strings are random
    // bytes that can hold anything, "/P" included.
    let dict = raw_object(input, id)
        .and_then(|mut body| {
            // The lexer reads operands up to an operator: give it one.
            body.extend(b" x");
            crate::content::parse(&body)
                .ok()?
                .into_iter()
                .next()?
                .args
                .into_iter()
                .next()
        })
        .and_then(|o| o.as_dict().ok().cloned())
        .unwrap_or_default();
    let get = |key: &str| dict.get(key.as_bytes()).ok().and_then(|v| v.as_i64().ok());
    let algorithm = match get("V") {
        Some(5) => "AES-256",
        Some(4) => "AES-128 or RC4-128",
        Some(2) | Some(3) => "RC4",
        Some(1) => "RC4-40",
        _ => "unknown",
    };
    Ok(SecurityInfo {
        encrypted: true,
        needs_password: doc.encryption_state.is_none(),
        revision: get("R"),
        algorithm: Some(algorithm.into()),
        permissions: get("P").map(PermissionSet::from_bits),
    })
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The last `/Encrypt n g R` in the file: the newest trailer wins.
fn encrypt_ref(input: &[u8]) -> Option<(u32, u16)> {
    let key = b"/Encrypt";
    let at = input.windows(key.len()).rposition(|w| w == key)?;
    let text =
        String::from_utf8_lossy(&input[at + key.len()..(at + key.len() + 32).min(input.len())])
            .into_owned();
    let mut parts = text.split_whitespace();
    let num = parts.next()?.parse().ok()?;
    let gen_r = parts.next()?;
    let generation = gen_r.trim_end_matches('R').parse().ok()?;
    Some((num, generation))
}

fn raw_object(input: &[u8], id: (u32, u16)) -> Option<Vec<u8>> {
    let header = format!("{} {} obj", id.0, id.1);
    let mut from = 0;
    while let Some(pos) = find(&input[from..], header.as_bytes()) {
        let at = from + pos;
        let boundary = at == 0 || !input[at - 1].is_ascii_digit();
        if boundary {
            let body = &input[at + header.len()..];
            let end = find(body, b"endobj").unwrap_or(body.len().min(4096));
            return Some(body[..end].to_vec());
        }
        from = at + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::testutil::{qpdf_check, qpdf_check_with};

    fn opts(user: &str, owner: Option<&str>) -> EncryptOptions {
        EncryptOptions {
            user_password: user.into(),
            owner_password: owner.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn roundtrip() {
        let input = fixtures::photo_pdf();
        let enc = encrypt(&input, &opts("secret", Some("owner"))).unwrap();
        qpdf_check_with(&enc, Some("secret"));
        assert_eq!(
            pdf::load(&enc, None).unwrap_err().code(),
            "password_required"
        );
        assert_eq!(
            pdf::load(&enc, Some("nope")).unwrap_err().code(),
            "wrong_password"
        );
        let info = inspect(&enc).unwrap();
        assert!(info.encrypted && info.needs_password);
        assert_eq!(info.algorithm.as_deref(), Some("AES-256"));
        assert_eq!(info.revision, Some(6));
        for pw in ["secret", "owner"] {
            let dec = decrypt(&enc, pw).unwrap();
            assert!(!inspect(&dec).unwrap().encrypted);
            assert_eq!(pdf::load(&dec, None).unwrap().get_pages().len(), 2);
            qpdf_check(&dec);
        }
    }

    #[test]
    fn unicode_password() {
        let enc = encrypt(&fixtures::text_pdf(1), &opts("pässwörd", None)).unwrap();
        assert!(decrypt(&enc, "pässwörd").is_ok());
    }

    #[test]
    fn permissions_only() {
        let o = EncryptOptions {
            user_password: String::new(),
            owner_password: Some("boss".into()),
            permissions: PermissionSet {
                print: false,
                copy: false,
                ..Default::default()
            },
            password: None,
        };
        let enc = encrypt(&fixtures::text_pdf(1), &o).unwrap();
        qpdf_check(&enc);
        let info = inspect(&enc).unwrap();
        assert!(!info.needs_password);
        let p = info.permissions.unwrap();
        assert!(!p.print && !p.copy && p.modify);
        // Opens without a password.
        assert_eq!(pdf::load(&enc, None).unwrap().get_pages().len(), 1);
    }

    #[test]
    fn permissions_survive_random_key_material() {
        // The /O, /U, /OE, /UE and /Perms strings are random bytes: whatever
        // they contain must not be mistaken for the /P entry.
        let input = fixtures::text_pdf(1);
        for _ in 0..1500 {
            let enc = encrypt(&input, &opts("", Some("boss"))).unwrap();
            let info = inspect(&enc).unwrap();
            assert!(info.permissions.is_some(), "permissions lost");
            assert_eq!(info.revision, Some(6));
        }
    }

    #[test]
    fn needs_some_password() {
        assert!(encrypt(&fixtures::text_pdf(1), &opts("", None)).is_err());
    }

    #[test]
    fn reencrypt_needs_old_password() {
        let enc = encrypt(&fixtures::text_pdf(1), &opts("a", None)).unwrap();
        let o = EncryptOptions {
            password: Some("a".into()),
            ..opts("b", None)
        };
        let again = encrypt(&enc, &o).unwrap();
        assert!(decrypt(&again, "b").is_ok());
        assert!(decrypt(&again, "a").is_err());
    }
}
