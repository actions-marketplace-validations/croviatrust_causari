//! Ed25519 key files under `.causari/keys/`.
//!
//! One routine for every identity the ledger owns (skill signing, seal
//! issuer, proof signing): the secret is written once, atomically, readable
//! by the owner only, with the public key next to it. Two protocols never
//! share a key; domain separation starts with the file name.

use anyhow::{Context, Result, anyhow};
use ed25519_dalek::SigningKey;
use std::path::{Path, PathBuf};

use crate::repo::Repo;

pub fn keys_dir(repo: &Repo) -> PathBuf {
    repo.dir.join("keys")
}

/// `.causari/keys/<name>.key` (secret, 0600) and `<name>.pub`.
pub fn key_path(repo: &Repo, name: &str) -> PathBuf {
    keys_dir(repo).join(format!("{name}.key"))
}

pub fn load(repo: &Repo, name: &str) -> Result<Option<SigningKey>> {
    let path = key_path(repo, name);
    if !path.exists() {
        return Ok(None);
    }
    let hex_str =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let bytes = hex::decode(hex_str.trim()).with_context(|| format!("decoding {name} key"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow!("{name} key must be 32 bytes"))?;
    Ok(Some(SigningKey::from_bytes(&arr)))
}

/// Load the named key, generating it on first use.
pub fn load_or_create(repo: &Repo, name: &str) -> Result<SigningKey> {
    if let Some(key) = load(repo, name)? {
        return Ok(key);
    }
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| anyhow!("generating {name} key: {e}"))?;
    let key = SigningKey::from_bytes(&secret);
    let dir = keys_dir(repo);
    std::fs::create_dir_all(&dir)?;
    write_secret(&key_path(repo, name), hex::encode(secret).as_bytes())?;
    write_atomic(
        &dir.join(format!("{name}.pub")),
        hex::encode(key.verifying_key().to_bytes()).as_bytes(),
    )?;
    Ok(key)
}

/// Write `data` to `path` through a temporary file in the same directory,
/// so a crash never leaves a half-written file behind.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent", path.display()))?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("key"),
        std::process::id()
    ));
    std::fs::write(&tmp, data).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renaming into {}", path.display()))?;
    Ok(())
}

/// Like `write_atomic`, owner-readable only where the platform supports it.
pub fn write_secret(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent", path.display()))?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("key"),
        std::process::id()
    ));
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .with_context(|| format!("creating {}", tmp.display()))?;
        std::io::Write::write_all(&mut f, data)?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path).with_context(|| format!("renaming into {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_created_once_and_reloaded() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let a = load_or_create(&repo, "proof-signing").unwrap();
        let b = load_or_create(&repo, "proof-signing").unwrap();
        assert_eq!(a.to_bytes(), b.to_bytes());
        assert!(
            key_path(&repo, "proof-signing")
                .with_extension("pub")
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn secret_is_owner_readable_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        load_or_create(&repo, "seal-issuer").unwrap();
        let mode = std::fs::metadata(key_path(&repo, "seal-issuer"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn distinct_names_are_distinct_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let a = load_or_create(&repo, "skill-signing").unwrap();
        let b = load_or_create(&repo, "proof-signing").unwrap();
        assert_ne!(a.to_bytes(), b.to_bytes());
    }
}
