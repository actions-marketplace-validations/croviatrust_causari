use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;

use crate::object::{Event, ObjectKind, Snapshot, Tree, canonical_json, hash_bytes};
use crate::repo::Repo;

fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Read-write access to the content-addressable object store.
pub struct Store<'a> {
    pub repo: &'a Repo,
}

/// What a slot in the object store holds, relative to the bytes a writer
/// wants to put there.
enum Probe {
    /// No file.
    Absent,
    /// Exactly the wanted bytes (by length + marker, or byte-for-byte).
    Present,
    /// A file that verifies as some *other* intact object under this id.
    Foreign,
    /// A file that verifies as nothing: truncated, zero-filled, tampered.
    Torn,
}

impl<'a> Store<'a> {
    pub fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.repo.objects_dir().join(&id[..2]).join(&id[2..])
    }

    /// True only for an object that is on disk AND verifies against its id.
    /// A torn or tampered file is not "present": callers that see `false`
    /// may rewrite it.
    #[allow(dead_code)] // public helper, used by tests and tooling
    pub fn exists(&self, id: &str) -> bool {
        valid_id(id) && self.read_raw(id).is_ok()
    }

    /// Write raw bytes and return the blob's id.
    ///
    /// Store format v2 hashes `'B' || content`, so a blob whose bytes are
    /// literally `T{...}` can never share an id with the tree `{...}`.
    /// Format v1 hashed bare `content`. To keep blob identity stable across
    /// the upgrade (otherwise every unchanged file of an existing repository
    /// would look modified in the first post-upgrade snapshot), a blob that
    /// already exists under its v1 id keeps that id. Only new content gets a
    /// v2 id. Both forms verify on read.
    pub fn write_blob(&self, content: &[u8]) -> Result<String> {
        let mut buf = Vec::with_capacity(content.len() + 1);
        buf.push(b'B');
        buf.extend_from_slice(content);
        let legacy_id = hash_bytes(content);
        match self.probe(&legacy_id, &buf) {
            // Intact v1 blob: keep its id.
            Probe::Present => return Ok(legacy_id),
            // The slot holds (or held) a v1 blob of exactly this content:
            // nothing else can hash there. Heal it so every tree that
            // references the v1 id becomes readable again.
            Probe::Torn => {
                self.put(&legacy_id, &buf)?;
                return Ok(legacy_id);
            }
            // Empty slot, or a structured object whose id happens to equal
            // hash(content): the blob takes its own v2 id.
            Probe::Absent | Probe::Foreign => {}
        }
        self.write_object(&buf)
    }

    /// Content-address `stored` (marker already prepended) and persist it.
    /// An existing intact object is left alone; a torn one is rewritten. If
    /// the slot holds an intact v1 blob whose *unmarked* bytes are exactly
    /// `stored` (a legacy store had `hash(content)` collide with a marked
    /// structured object, F02) we refuse to alias it.
    fn write_object(&self, stored: &[u8]) -> Result<String> {
        let id = hash_bytes(stored);
        match self.probe(&id, stored) {
            Probe::Present => Ok(id),
            Probe::Absent | Probe::Torn => {
                self.put(&id, stored)?;
                Ok(id)
            }
            Probe::Foreign => Err(anyhow!(
                "object {} already exists with different content (legacy v1 blob colliding with a {:?} object); store integrity violation",
                id,
                stored[0] as char
            )),
        }
    }

    /// Classify what is on disk at `id`, relative to the bytes `stored` we
    /// want there. The fast path is a stat plus a one-byte read: a write
    /// that is torn by truncation, or zero-filled after a crash, fails the
    /// length or marker check without hashing the whole object. Only on a
    /// mismatch is the file read and hashed to tell a torn copy from an
    /// intact object of another kind.
    fn probe(&self, id: &str, stored: &[u8]) -> Probe {
        let path = self.path_for(id);
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return Probe::Absent,
        };
        if meta.len() == stored.len() as u64 {
            let mut first = [0u8; 1];
            let marker_ok = std::fs::File::open(&path)
                .and_then(|mut f| {
                    use std::io::Read;
                    f.read_exact(&mut first)
                })
                .map(|_| first[0] == stored[0])
                .unwrap_or(false);
            if marker_ok {
                return Probe::Present;
            }
        }
        let raw = match std::fs::read(&path) {
            Ok(r) => r,
            Err(_) => return Probe::Absent,
        };
        if raw == stored {
            return Probe::Present;
        }
        let intact_other = match raw.first() {
            Some(b'B') => hash_bytes(&raw[1..]) == id || hash_bytes(&raw) == id,
            Some(b'T' | b'S' | b'E') => hash_bytes(&raw) == id,
            _ => false,
        };
        if intact_other {
            Probe::Foreign
        } else {
            Probe::Torn
        }
    }

    /// Persist `stored` at `id` via temp file + rename in the bucket
    /// directory, so no reader can ever observe a half-written object under
    /// its final name. Scratch files start with `.`, which no hex prefix can
    /// match, so `resolve_id`'s bucket scan never picks one up.
    fn put(&self, id: &str, stored: &[u8]) -> Result<()> {
        let path = self.path_for(id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::keys::write_atomic(&path, stored).with_context(|| format!("writing object {}", id))
    }

    fn write_structured<T: serde::Serialize>(&self, kind: ObjectKind, value: &T) -> Result<String> {
        let json = canonical_json(value)?;
        // Compose stored bytes as: marker byte + canonical json
        let marker = match kind {
            ObjectKind::Tree => b'T',
            ObjectKind::Snapshot => b'S',
            ObjectKind::Event => b'E',
            ObjectKind::Blob => return Err(anyhow!("use write_blob for blobs")),
        };
        let mut to_hash = Vec::with_capacity(json.len() + 1);
        to_hash.push(marker);
        to_hash.extend_from_slice(&json);
        self.write_object(&to_hash)
    }

    pub fn write_tree(&self, tree: &Tree) -> Result<String> {
        self.write_structured(ObjectKind::Tree, tree)
    }

    pub fn write_snapshot(&self, snapshot: &Snapshot) -> Result<String> {
        self.write_structured(ObjectKind::Snapshot, snapshot)
    }

    pub fn write_event(&self, event: &Event) -> Result<String> {
        self.write_structured(ObjectKind::Event, event)
    }

    fn read_raw(&self, id: &str) -> Result<Vec<u8>> {
        if !valid_id(id) {
            return Err(anyhow!(
                "invalid object id: expected 64 lowercase hexadecimal characters"
            ));
        }
        let path = self.path_for(id);
        let raw = std::fs::read(&path).with_context(|| format!("reading object {}", id))?;
        let intact = match raw.first() {
            // Blob: v2 hashes the marked bytes, v1 hashed the bare content.
            Some(b'B') => hash_bytes(&raw) == id || hash_bytes(&raw[1..]) == id,
            Some(b'T' | b'S' | b'E') => hash_bytes(&raw) == id,
            _ => {
                return Err(anyhow!(
                    "object {} has an invalid or missing kind marker",
                    id
                ));
            }
        };
        if !intact {
            return Err(anyhow!(
                "object {} failed integrity verification (BLAKE3 mismatch)",
                id
            ));
        }
        Ok(raw)
    }

    fn read_structured<T: serde::de::DeserializeOwned>(
        &self,
        id: &str,
        expected_marker: u8,
    ) -> Result<T> {
        let raw = self.read_raw(id)?;
        if raw.is_empty() {
            return Err(anyhow!("empty object {}", id));
        }
        if raw[0] != expected_marker {
            return Err(anyhow!(
                "object {} has wrong kind (expected marker {:?}, got {:?})",
                id,
                expected_marker as char,
                raw[0] as char
            ));
        }
        let value =
            serde_json::from_slice(&raw[1..]).with_context(|| format!("parsing object {}", id))?;
        Ok(value)
    }

    pub fn read_blob(&self, id: &str) -> Result<Vec<u8>> {
        let raw = self.read_raw(id)?;
        if raw.is_empty() || raw[0] != b'B' {
            return Err(anyhow!("object {} is not a blob", id));
        }
        Ok(raw[1..].to_vec())
    }

    pub fn read_tree(&self, id: &str) -> Result<Tree> {
        self.read_structured(id, b'T')
    }

    pub fn read_snapshot(&self, id: &str) -> Result<Snapshot> {
        self.read_structured(id, b'S')
    }

    pub fn read_event(&self, id: &str) -> Result<Event> {
        self.read_structured(id, b'E')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::TreeEntry;
    use std::collections::BTreeMap;

    fn test_repo() -> (tempfile::TempDir, Repo) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        (tmp, repo)
    }

    fn sample_event(parent: Option<String>) -> Event {
        Event {
            schema: "causari.event.v0.2".into(),
            parent,
            agent: Some("test-agent".into()),
            model: None,
            tool: Some("edit".into()),
            message: Some("hello".into()),
            prompt: Some("do the thing".into()),
            reasoning: None,
            reads: vec![],
            writes: vec!["a.txt".into()],
            tokens_in: None,
            tokens_out: None,
            cost_usd: None,
            pre_snapshot: "pre".into(),
            post_snapshot: "post".into(),
            exit_code: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            evidence: None,
        }
    }

    #[test]
    fn malformed_ids_return_errors_without_panicking() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        for id in ["", "a", "é", "../outside", &"z".repeat(64)] {
            assert!(store.read_blob(id).is_err(), "accepted {id:?}");
            assert!(!store.exists(id));
        }
    }

    #[test]
    fn tampered_objects_are_rejected() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        let id = store.write_blob(b"original").unwrap();
        std::fs::write(store.path_for(&id), b"Btampered").unwrap();
        assert!(store.read_blob(&id).is_err());
        let tree = store
            .write_tree(&Tree {
                entries: BTreeMap::new(),
            })
            .unwrap();
        std::fs::write(store.path_for(&tree), b"T{\"entries\":{},\"extra\":true}").unwrap();
        assert!(store.read_tree(&tree).is_err());
    }

    #[test]
    fn blob_roundtrip_and_dedup() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        let id1 = store.write_blob(b"hello world").unwrap();
        let id2 = store.write_blob(b"hello world").unwrap();
        assert_eq!(id1, id2, "identical content must dedup to one object");
        assert_eq!(store.read_blob(&id1).unwrap(), b"hello world");
        assert!(store.exists(&id1));
    }

    #[test]
    fn structured_roundtrips() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        let mut entries = BTreeMap::new();
        entries.insert(
            "main.rs".to_string(),
            TreeEntry::blob("ff".repeat(32), false),
        );
        let tree_id = store.write_tree(&Tree { entries }).unwrap();
        let tree = store.read_tree(&tree_id).unwrap();
        assert_eq!(tree.entries["main.rs"].kind, "blob");

        let snap_id = store
            .write_snapshot(&Snapshot {
                tree: tree_id.clone(),
                created_at: "2026-01-01T00:00:00Z".into(),
            })
            .unwrap();
        assert_eq!(store.read_snapshot(&snap_id).unwrap().tree, tree_id);

        let ev_id = store.write_event(&sample_event(None)).unwrap();
        let ev = store.read_event(&ev_id).unwrap();
        assert_eq!(ev.message.as_deref(), Some("hello"));
        assert_eq!(ev.parent, None);
    }

    #[test]
    fn kind_markers_prevent_cross_kind_reads() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        let blob_id = store.write_blob(b"{}").unwrap();
        assert!(store.read_tree(&blob_id).is_err());
        assert!(store.read_snapshot(&blob_id).is_err());
        assert!(store.read_event(&blob_id).is_err());

        let ev_id = store.write_event(&sample_event(None)).unwrap();
        assert!(store.read_blob(&ev_id).is_err());
        assert!(store.read_tree(&ev_id).is_err());
    }

    #[test]
    fn blob_and_structured_with_same_bytes_do_not_collide() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        // A blob whose content happens to be a tree's canonical JSON must
        // still get a different id (the kind marker is hashed).
        let tree = Tree {
            entries: BTreeMap::new(),
        };
        let tree_id = store.write_tree(&tree).unwrap();
        let json = crate::object::canonical_json(&tree).unwrap();
        let blob_id = store.write_blob(&json).unwrap();
        assert_ne!(tree_id, blob_id);
    }

    #[test]
    fn legacy_v1_blob_keeps_its_id_after_upgrade() {
        // Store format v1 wrote 'B'||content under hash(content). Re-writing
        // the same content on a v2 binary must return the v1 id, otherwise
        // every unchanged file looks modified in the first snapshot after
        // the upgrade and the causal metadata of that event is wrong.
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        let content = b"fn main() { println!(\"hello\"); }";
        let v1_id = hash_bytes(content);
        let path = store.path_for(&v1_id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut v1_bytes = vec![b'B'];
        v1_bytes.extend_from_slice(content);
        std::fs::write(&path, &v1_bytes).unwrap();

        assert_eq!(store.write_blob(content).unwrap(), v1_id);
        assert_eq!(store.read_blob(&v1_id).unwrap(), content);

        // Brand-new content gets a v2 id and verifies too.
        let fresh = store.write_blob(b"new file").unwrap();
        assert_ne!(fresh, hash_bytes(b"new file"));
        assert_eq!(store.read_blob(&fresh).unwrap(), b"new file");
    }

    #[test]
    fn blob_whose_bytes_are_a_marked_tree_does_not_alias_the_tree() {
        // Regression (F02): a file containing exactly `T{"entries":{}}` used
        // to hash to the same id as the empty tree object, so recording
        // succeeded and the next restore failed with "is not a blob".
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        let tree = Tree {
            entries: BTreeMap::new(),
        };
        let tree_id = store.write_tree(&tree).unwrap();
        let mut evil = vec![b'T'];
        evil.extend_from_slice(&crate::object::canonical_json(&tree).unwrap());

        let blob_id = store.write_blob(&evil).unwrap();
        assert_ne!(tree_id, blob_id);
        assert_eq!(store.read_blob(&blob_id).unwrap(), evil);
        assert!(store.read_tree(&tree_id).is_ok());

        // And the other way round: writing the blob first must not poison
        // the tree id either.
        let (_tmp2, repo2) = test_repo();
        let store2 = Store::new(&repo2);
        let blob_first = store2.write_blob(&evil).unwrap();
        let tree_after = store2.write_tree(&tree).unwrap();
        assert_ne!(blob_first, tree_after);
        assert!(store2.read_tree(&tree_after).is_ok());
    }

    #[test]
    fn truncated_blob_is_healed_by_rewriting_the_same_content() {
        // Regression (B1): a torn object used to count as present because
        // only "exists + first byte" was checked, so the content could never
        // be written again and every read failed integrity.
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        let content = b"fn main() { println!(\"torn\"); }".repeat(8);
        let id = store.write_blob(&content).unwrap();
        let path = store.path_for(&id);

        // Truncated mid-write.
        let full = std::fs::read(&path).unwrap();
        std::fs::write(&path, &full[..full.len() / 2]).unwrap();
        assert!(store.read_blob(&id).is_err());
        assert!(!store.exists(&id));

        assert_eq!(store.write_blob(&content).unwrap(), id);
        assert_eq!(store.read_blob(&id).unwrap(), content);
        assert!(store.exists(&id));

        // Zero-length (crash before any byte landed).
        std::fs::write(&path, b"").unwrap();
        assert_eq!(store.write_blob(&content).unwrap(), id);
        assert_eq!(store.read_blob(&id).unwrap(), content);

        // Same length, wrong bytes (zero-filled after a power loss).
        std::fs::write(&path, vec![0u8; full.len()]).unwrap();
        assert_eq!(store.write_blob(&content).unwrap(), id);
        assert_eq!(store.read_blob(&id).unwrap(), content);
    }

    #[test]
    fn truncated_structured_objects_are_healed_too() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        let mut entries = BTreeMap::new();
        entries.insert("x".to_string(), TreeEntry::blob("ab".repeat(32), false));
        let tree = Tree { entries };
        let id = store.write_tree(&tree).unwrap();
        std::fs::write(store.path_for(&id), b"T{\"entr").unwrap();
        assert!(store.read_tree(&id).is_err());
        assert_eq!(store.write_tree(&tree).unwrap(), id);
        assert_eq!(store.read_tree(&id).unwrap().entries.len(), 1);

        let ev = sample_event(None);
        let ev_id = store.write_event(&ev).unwrap();
        std::fs::write(store.path_for(&ev_id), b"").unwrap();
        assert_eq!(store.write_event(&ev).unwrap(), ev_id);
        assert!(store.read_event(&ev_id).is_ok());
    }

    #[test]
    fn truncated_legacy_v1_blob_is_healed_under_its_v1_id() {
        // Old trees reference the v1 id; healing must restore it there, not
        // create a fresh v2 copy and leave the old references dangling.
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        let content = b"legacy content that got torn";
        let v1_id = hash_bytes(content);
        let path = store.path_for(&v1_id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"Blegacy").unwrap();
        assert!(store.read_blob(&v1_id).is_err());

        assert_eq!(store.write_blob(content).unwrap(), v1_id);
        assert_eq!(store.read_blob(&v1_id).unwrap(), content);
    }

    #[test]
    fn object_writes_leave_no_scratch_files_behind() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        let id = store.write_blob(b"scratch").unwrap();
        let bucket = store.path_for(&id).parent().unwrap().to_path_buf();
        let names: Vec<String> = std::fs::read_dir(&bucket)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![id[2..].to_string()]);
        // A short prefix still resolves uniquely inside the bucket.
        assert_eq!(
            crate::object::resolve_id(&repo.objects_dir(), &id[..8]).unwrap(),
            id
        );
    }

    #[test]
    fn reading_missing_object_fails_cleanly() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        assert!(store.read_event(&"a".repeat(64)).is_err());
        assert!(!store.exists(&"a".repeat(64)));
    }
}
