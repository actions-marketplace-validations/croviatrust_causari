//! PNX runs on disk: `.causari/pnx/<run_id>/`.
//!
//! ```text
//! meta.json         salt, parameters, counts, timestamps; rewritten atomically per body
//! fingerprints.log  one salted fingerprint per line (hex), appended as bodies arrive
//! sheet.json        the signed run sheet, written once when the run is closed (public)
//! proof.json        default output of `re pnx prove`
//! ```
//!
//! Nothing under this directory is traffic or asset bytes: fingerprints are
//! salted SHA-256 digests of 32-byte windows. They still allow membership
//! tests against guessed strings once the salt is known (the salt is in the
//! public sheet), so the log is written owner-readable and `.causari/` stays
//! gitignored. Only `sheet.json` and a proof are meant to leave the machine.
//!
//! Ordering matters for the claim the sheet makes. The proxy records a body
//! *before* forwarding it: fingerprints are appended to the log, `meta.json`
//! is rewritten, and only then do the bytes go upstream. A crash in between
//! leaves fingerprints of a body that never left — the map over-commits,
//! which can only turn an `absent` into a `present`, never the reverse.

use anyhow::{Context, Result, anyhow, bail};
use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::keys;
use crate::pnx::{Hash, PROFILE, Params, Witness};
use crate::repo::Repo;

/// Name of the dedicated witness key under `.causari/keys/`. Never the seal
/// issuer key: a sheet and a receipt are different statements.
pub const WITNESS_KEY_NAME: &str = "pnx-witness";
const META_FORMAT: &str = "causari.pnx.run.v1";
const META_FILE: &str = "meta.json";
const LOG_FILE: &str = "fingerprints.log";
const SHEET_FILE: &str = "sheet.json";
const PROOF_FILE: &str = "proof.json";

pub fn pnx_dir(repo: &Repo) -> PathBuf {
    repo.dir.join("pnx")
}

pub fn run_dir(repo: &Repo, run_id: &str) -> PathBuf {
    pnx_dir(repo).join(run_id)
}

/// Run ids name a directory, so they are restricted to a portable subset.
pub fn validate_run_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if !ok {
        bail!("run id {id:?} must be 1-128 characters of [A-Za-z0-9._-] and not start with '.'");
    }
    Ok(())
}

/// `pnx-YYYYMMDD-HHMMSS-<6 hex>`: sortable, unique enough for one machine.
pub fn new_run_id() -> Result<String> {
    let mut raw = [0u8; 3];
    getrandom::fill(&mut raw).map_err(|e| anyhow!("secure randomness unavailable: {e}"))?;
    Ok(format!(
        "pnx-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        hex::encode(raw)
    ))
}

pub fn new_salt() -> Result<[u8; 16]> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| anyhow!("secure randomness unavailable: {e}"))?;
    Ok(salt)
}

/// RFC 3339, UTC, second precision: what the sheet carries.
pub fn now_rfc3339() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// `urn:crovia:pnx-witness:causari:<first 12 hex of the public key>`.
pub fn witness_id(key: &SigningKey) -> String {
    let pk = hex::encode(key.verifying_key().to_bytes());
    format!("urn:crovia:pnx-witness:causari:{}", &pk[..12])
}

/// The repository's witness key, created on first use.
pub fn witness_key(repo: &Repo) -> Result<SigningKey> {
    keys::load_or_create(repo, WITNESS_KEY_NAME)
}

/// One run directory with its witness state loaded.
pub struct Run {
    dir: PathBuf,
    pub witness: Witness,
    opened_at: String,
}

impl Run {
    pub fn run_id(&self) -> &str {
        &self.witness.run_id
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn sheet_path(&self) -> PathBuf {
        self.dir.join(SHEET_FILE)
    }

    pub fn proof_path(&self) -> PathBuf {
        self.dir.join(PROOF_FILE)
    }

    pub fn is_closed(&self) -> bool {
        self.sheet_path().exists()
    }

    /// Create a run for witnessing, or resume one that was opened and never
    /// closed (the proxy died). A closed run is refused: its sheet is signed.
    pub fn open(repo: &Repo, run_id: &str, salt: [u8; 16]) -> Result<(Self, bool)> {
        validate_run_id(run_id)?;
        let dir = run_dir(repo, run_id);
        if dir.join(META_FILE).exists() {
            let run = Self::load_dir(&dir)?;
            if run.is_closed() {
                bail!(
                    "run {run_id:?} is already closed ({}); choose another --run-id",
                    run.sheet_path().display()
                );
            }
            return Ok((run, true));
        }
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let run = Self {
            dir,
            witness: Witness::new(run_id, salt),
            opened_at: now_rfc3339(),
        };
        run.write_meta()?;
        Ok((run, false))
    }

    /// Load a run by id from the repository, open or closed.
    pub fn load(repo: &Repo, run_id: &str) -> Result<Self> {
        validate_run_id(run_id)?;
        Self::load_dir(&run_dir(repo, run_id))
    }

    /// Load a run from its directory (`meta.json` + `fingerprints.log`).
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let meta_path = dir.join(META_FILE);
        let meta: Value = serde_json::from_str(
            &std::fs::read_to_string(&meta_path)
                .with_context(|| format!("no PNX run at {}", dir.display()))?,
        )
        .with_context(|| format!("parsing {}", meta_path.display()))?;
        if meta.get("format").and_then(Value::as_str) != Some(META_FORMAT) {
            bail!("{}: not a {META_FORMAT} file", meta_path.display());
        }
        let run_id = meta["run_id"]
            .as_str()
            .ok_or_else(|| anyhow!("meta.json: missing run_id"))?;
        let salt: [u8; 16] = hex::decode(meta["salt_hex"].as_str().unwrap_or(""))
            .ok()
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| anyhow!("meta.json: salt_hex must be 16 bytes of hex"))?;
        let mut witness = Witness::new(run_id, salt);
        witness.params = Params {
            k_gram: meta["k_gram"].as_u64().unwrap_or(0) as usize,
            window: meta["window"].as_u64().unwrap_or(0) as usize,
        };
        if witness.params.k_gram == 0 || witness.params.window == 0 {
            bail!("meta.json: invalid k_gram/window");
        }
        witness.normalization = meta["normalization"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        witness.bodies = meta["bodies"].as_u64().unwrap_or(0);
        witness.bytes = meta["bytes"].as_u64().unwrap_or(0);
        witness.first_at = meta["first_at"].as_str().map(String::from);
        witness.last_at = meta["last_at"].as_str().map(String::from);

        let log_path = dir.join(LOG_FILE);
        if log_path.exists() {
            let raw = std::fs::read_to_string(&log_path)
                .with_context(|| format!("reading {}", log_path.display()))?;
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let fp: Hash = hex::decode(line)
                    .ok()
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| anyhow!("{}: corrupt fingerprint line", log_path.display()))?;
                witness.map.insert(fp);
            }
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            witness,
            opened_at: meta["opened_at"].as_str().unwrap_or_default().to_string(),
        })
    }

    fn write_meta(&self) -> Result<()> {
        let mut normalization = self.witness.normalization.clone();
        normalization.sort();
        let meta = json!({
            "format": META_FORMAT,
            "profile": PROFILE,
            "run_id": self.witness.run_id,
            "salt_hex": hex::encode(self.witness.salt),
            "k_gram": self.witness.params.k_gram,
            "window": self.witness.params.window,
            "normalization": normalization,
            "opened_at": self.opened_at,
            "bodies": self.witness.bodies,
            "bytes": self.witness.bytes,
            "first_at": self.witness.first_at,
            "last_at": self.witness.last_at,
        });
        keys::write_atomic(
            &self.dir.join(META_FILE),
            serde_json::to_string_pretty(&meta)?.as_bytes(),
        )
    }

    /// Fingerprint one outbound body and persist before returning, so the
    /// caller can forward the bytes knowing they are committed. Returns the
    /// number of fingerprints new to this run.
    pub fn ingest(&mut self, body: &[u8], at: &str) -> Result<usize> {
        if self.is_closed() {
            bail!("run {:?} is closed", self.witness.run_id);
        }
        let added = self.witness.ingest(body, at);
        if !added.is_empty() {
            let mut text = String::with_capacity(added.len() * 65);
            for fp in &added {
                text.push_str(&hex::encode(fp));
                text.push('\n');
            }
            append_private(&self.dir.join(LOG_FILE), text.as_bytes())?;
        }
        self.write_meta()?;
        Ok(added.len())
    }

    /// Sign the run sheet with `key` and write `sheet.json`. Once.
    pub fn close(&mut self, key: &SigningKey) -> Result<Value> {
        if self.is_closed() {
            bail!(
                "run {:?} is already closed ({})",
                self.witness.run_id,
                self.sheet_path().display()
            );
        }
        let sheet = self.witness.sheet(key, &witness_id(key), &now_rfc3339())?;
        keys::write_atomic(
            &self.sheet_path(),
            format!("{}\n", serde_json::to_string_pretty(&sheet)?).as_bytes(),
        )?;
        Ok(sheet)
    }

    /// The signed sheet of a closed run.
    pub fn sheet(&self) -> Result<Value> {
        let path = self.sheet_path();
        let raw = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "run {:?} is not closed yet (no {})",
                self.witness.run_id,
                path.display()
            )
        })?;
        crate::seal::parse_json_strict(&raw).with_context(|| format!("parsing {}", path.display()))
    }
}

/// Append to an owner-readable file, creating it with mode 0600 where the
/// platform supports it.
fn append_private(path: &Path, data: &[u8]) -> Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    f.write_all(data)?;
    f.flush()?;
    Ok(())
}

/// One line of `re pnx list`.
pub struct RunSummary {
    pub run_id: String,
    pub opened_at: String,
    pub closed: bool,
    pub bodies: u64,
    pub bytes: u64,
    pub fingerprints: usize,
    pub root: Option<String>,
}

/// Every run under `.causari/pnx/`, oldest first.
pub fn list_runs(repo: &Repo) -> Result<Vec<RunSummary>> {
    let dir = pnx_dir(repo);
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !path.join(META_FILE).exists() {
            continue;
        }
        let run = match Run::load_dir(&path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", path.display());
                continue;
            }
        };
        let root = if run.is_closed() {
            run.sheet()
                .ok()
                .and_then(|s| s["root"].as_str().map(String::from))
        } else {
            None
        };
        out.push(RunSummary {
            run_id: run.run_id().to_string(),
            opened_at: run.opened_at.clone(),
            closed: run.is_closed(),
            bodies: run.witness.bodies,
            bytes: run.witness.bytes,
            fingerprints: run.witness.map.len(),
            root,
        });
    }
    out.sort_by(|a, b| a.opened_at.cmp(&b.opened_at).then(a.run_id.cmp(&b.run_id)));
    Ok(out)
}

/// The most recently opened run that has no sheet yet.
pub fn latest_open_run(repo: &Repo) -> Result<Option<String>> {
    Ok(list_runs(repo)?
        .into_iter()
        .rev()
        .find(|r| !r.closed)
        .map(|r| r.run_id))
}

/// Resolve `--run <run_id|path-to-sheet.json|path-to-run-dir>` to a run.
pub fn resolve_run(repo: Option<&Repo>, spec: &str) -> Result<Run> {
    let p = Path::new(spec);
    if p.is_file() {
        let dir = p
            .parent()
            .ok_or_else(|| anyhow!("{spec}: no parent directory"))?;
        return Run::load_dir(dir);
    }
    if p.is_dir() && p.join(META_FILE).exists() {
        return Run::load_dir(p);
    }
    let repo = repo.ok_or_else(|| {
        anyhow!("{spec}: not a run directory, and no causari repository here to look up a run id")
    })?;
    Run::load(repo, spec)
}

/// What a witness fingerprints, for status lines.
pub fn describe_layers(w: &Witness) -> String {
    if w.normalization.is_empty() {
        "raw bytes only".to_string()
    } else {
        format!("raw bytes + {}", w.normalization.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pnx::{Params, verify_proof, verify_sheet};
    use std::collections::BTreeMap;

    #[test]
    fn run_ids_are_portable_directory_names() {
        assert!(validate_run_id("pnx-20260920-101010-ab12cd").is_ok());
        assert!(validate_run_id("ci-4711.agent_review").is_ok());
        assert!(validate_run_id("").is_err());
        assert!(validate_run_id(".hidden").is_err());
        assert!(validate_run_id("a/b").is_err());
        assert!(validate_run_id("a b").is_err());
        assert!(validate_run_id(&"x".repeat(129)).is_err());
        assert!(validate_run_id(&new_run_id().unwrap()).is_ok());
    }

    #[test]
    fn a_run_survives_reload_and_closes_once() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let salt = [3u8; 16];
        let (mut run, resumed) = Run::open(&repo, "run-a", salt).unwrap();
        assert!(!resumed);
        let body = serde_json::to_vec(&json!({"messages": [{"role": "user",
            "content": "please review this long enough string of configuration text"}]}))
        .unwrap();
        let n = run.ingest(&body, "2026-09-20T10:00:00Z").unwrap();
        assert!(n > 0);
        assert_eq!(run.ingest(&body, "2026-09-20T10:00:01Z").unwrap(), 0);
        assert_eq!(run.witness.bodies, 2);
        let root_before = run.witness.root();

        // Reloading rebuilds the same map from meta + log; the log holds
        // fingerprints only.
        let (mut again, resumed) = Run::open(&repo, "run-a", [9u8; 16]).unwrap();
        assert!(resumed);
        assert_eq!(again.witness.salt, salt, "resume keeps the recorded salt");
        assert_eq!(again.witness.bodies, 2);
        assert_eq!(again.witness.bytes, 2 * body.len() as u64);
        assert_eq!(again.witness.root(), root_before);
        let log = std::fs::read_to_string(again.dir().join(LOG_FILE)).unwrap();
        assert!(
            log.lines()
                .all(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
        );
        assert!(!log.contains("review"));

        let key = witness_key(&repo).unwrap();
        let sheet = again.close(&key).unwrap();
        assert!(verify_sheet(&sheet).is_empty());
        assert_eq!(sheet["witness"]["id"], witness_id(&key));
        assert_eq!(sheet["egress"]["bodies"], 2);
        assert!(again.is_closed());
        assert!(again.close(&key).is_err());
        assert!(again.ingest(b"more", "2026-09-20T10:00:02Z").is_err());
        assert!(
            Run::open(&repo, "run-a", salt).is_err(),
            "closed runs are not reopened"
        );

        // The witness key is its own identity, never the seal issuer's.
        let seal_key = keys::load_or_create(&repo, "seal-issuer").unwrap();
        assert_ne!(seal_key.to_bytes(), key.to_bytes());

        // Prove against the stored sheet after a cold load.
        let mut loaded = Run::load(&repo, "run-a").unwrap();
        let stored = loaded.sheet().unwrap();
        let asset = b"please review this long enough string of configuration text".to_vec();
        let proof = loaded
            .witness
            .prove(&stored, &[("prompt".to_string(), asset.clone())])
            .unwrap();
        let mut supplied = BTreeMap::new();
        supplied.insert("prompt".to_string(), asset);
        let res = verify_proof(&proof, Some(&supplied));
        assert!(res.ok && res.verdict == "present");

        let runs = list_runs(&repo).unwrap();
        assert_eq!(runs.len(), 1);
        assert!(runs[0].closed && runs[0].root.is_some());
        assert_eq!(latest_open_run(&repo).unwrap(), None);
        let (_, _) = Run::open(&repo, "run-b", salt).unwrap();
        assert_eq!(latest_open_run(&repo).unwrap().as_deref(), Some("run-b"));

        // Resolution by id, by directory and by sheet path.
        assert_eq!(resolve_run(Some(&repo), "run-a").unwrap().run_id(), "run-a");
        let dir = run_dir(&repo, "run-a");
        assert_eq!(
            resolve_run(None, dir.to_str().unwrap()).unwrap().run_id(),
            "run-a"
        );
        let sheet_path = dir.join(SHEET_FILE);
        assert_eq!(
            resolve_run(None, sheet_path.to_str().unwrap())
                .unwrap()
                .run_id(),
            "run-a"
        );
        assert!(resolve_run(None, "run-a").is_err());
        assert_eq!(loaded.witness.params, Params::DEFAULT);
    }

    #[cfg(unix)]
    #[test]
    fn the_fingerprint_log_is_owner_readable_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        let (mut run, _) = Run::open(&repo, "perm", [1u8; 16]).unwrap();
        run.ingest(&[b'z'; 100], "2026-09-20T10:00:00Z").unwrap();
        let mode = std::fs::metadata(run.dir().join(LOG_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
