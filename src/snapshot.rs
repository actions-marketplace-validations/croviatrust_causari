use anyhow::{Context, Result, bail};
use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::object::{Tree, TreeEntry};
use crate::repo::Repo;
use crate::store::Store;

/// Never captured, at any depth, whatever `.gitignore` says: the ledger
/// itself, git's own store, and dotenv files.
const ALWAYS_EXCLUDED: &[&str] = &[".causari", ".git"];

/// Built-in exclusions. In a git work tree they apply at the top level only
/// (lowest precedence, so a `!/dist/` line in `.gitignore` re-includes a
/// tracked `dist/`); outside git they are the whole rule, see `is_ignored`.
const DEFAULT_IGNORES: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".venv",
    "__pycache__",
    ".idea",
    ".vscode",
];

/// Well-known build-output directory names, excluded at any depth by the
/// cheap rule (`is_ignored`) because they are huge and churn constantly.
const BUILD_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".venv",
    "__pycache__",
];

/// Dotenv files usually hold secrets (API keys, DB URLs). Keep `.env` and its
/// variants (`.env.local`, `.env.production`, …) out of snapshots by default,
/// so credentials are never copied into the `.causari/` ledger.
fn is_secret_env_file(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.")
}

/// A name that no rule set may ever capture or restore.
fn is_protected_name(name: &str) -> bool {
    ALWAYS_EXCLUDED.contains(&name) || is_secret_env_file(name)
}

/// Cheap, path-only exclusion check.
///
/// This is the complete rule when the workspace is not a git work tree, and
/// a conservative approximation otherwise: it knows the built-in list but
/// does not read `.gitignore`. `re watch` uses it to decide whether a
/// filesystem event deserves a snapshot at all; the snapshot itself applies
/// the full rules ([`IgnoreRules`]), so a path this function lets through is
/// still subject to `.gitignore`, and a nested build directory it drops
/// (`src/build/`) is simply not re-snapshotted until another event fires.
///
/// Rule: protected names anywhere; the built-in list at the top level;
/// the well-known build directories at any depth.
pub fn is_ignored(rel_path: &Path) -> bool {
    let mut comps = rel_path.components().filter_map(|c| c.as_os_str().to_str());
    let Some(first) = comps.next() else {
        return false;
    };
    if is_protected_name(first) || DEFAULT_IGNORES.contains(&first) {
        return true;
    }
    comps.any(|c| is_protected_name(c) || BUILD_DIRS.contains(&c))
}

/// What a snapshot leaves out, and therefore what a restore must not delete.
///
/// In a git work tree (`<root>/.git` exists) this follows
/// `git ls-files --cached --others --exclude-standard`: nested `.gitignore`
/// files, `.git/info/exclude` and the global excludes file, with the usual
/// precedence (deeper file wins, last matching line wins, `!` re-includes).
/// Two additions: the protected names are excluded regardless, and the
/// built-in list is applied at the top level as the lowest-precedence layer.
/// Git's index is not consulted, so a tracked top-level `dist/` needs a
/// `!/dist/` line to be captured. Without `.git`, the rule is `is_ignored`.
pub struct IgnoreRules {
    /// Lowest precedence first: built-ins, global excludes, info/exclude.
    /// Per-directory `.gitignore` files are pushed on top during a walk.
    /// `None` outside a git work tree.
    base: Option<Vec<Gitignore>>,
}

impl IgnoreRules {
    pub fn for_root(root: &Path) -> Self {
        if !root.join(".git").exists() {
            return Self { base: None };
        }
        let mut base = Vec::new();
        let mut builtins = GitignoreBuilder::new(root);
        for name in DEFAULT_IGNORES {
            let _ = builtins.add_line(None, &format!("/{}/", name));
        }
        if let Ok(gi) = builtins.build() {
            base.push(gi);
        }
        let (global, _) = GitignoreBuilder::new(root).build_global();
        if !global.is_empty() {
            base.push(global);
        }
        let exclude = root.join(".git").join("info").join("exclude");
        if exclude.is_file() {
            let mut b = GitignoreBuilder::new(root);
            let _ = b.add(&exclude);
            if let Ok(gi) = b.build() {
                base.push(gi);
            }
        }
        Self { base: Some(base) }
    }

    /// Start a walk at `root`: the base layers plus the root `.gitignore`.
    fn root_stack(&self, root: &Path) -> Vec<Gitignore> {
        let mut stack = self.base.clone().unwrap_or_default();
        self.push_dir(root, &mut stack);
        stack
    }

    /// Push `dir/.gitignore` onto the stack if present. Returns whether a
    /// layer was pushed, so the caller can pop it on the way out.
    fn push_dir(&self, dir: &Path, stack: &mut Vec<Gitignore>) -> bool {
        if self.base.is_none() {
            return false;
        }
        let file = dir.join(".gitignore");
        if !file.is_file() {
            return false;
        }
        let (gi, _err) = Gitignore::new(&file);
        stack.push(gi);
        true
    }

    /// Is the directory entry `name` at `path` (relative `rel`) excluded?
    fn excluded(
        &self,
        stack: &[Gitignore],
        path: &Path,
        rel: &Path,
        name: &str,
        is_dir: bool,
    ) -> bool {
        if is_protected_name(name) {
            return true;
        }
        if self.base.is_none() {
            return is_ignored(rel);
        }
        for gi in stack.iter().rev() {
            match gi.matched(path, is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }
}

/// Visit every capturable, non-excluded entry under `root` as
/// `(absolute path, workspace-relative path, is_dir)`. Directories are
/// reported after their contents, so a caller can remove what became empty.
/// Skips whole excluded directories, symlinks and uncapturable names: the
/// same set of entries a snapshot would record.
fn walk_entries(
    rules: &IgnoreRules,
    root: &Path,
    f: &mut dyn FnMut(&Path, &Path, bool) -> Result<()>,
) -> Result<()> {
    let mut stack = rules.root_stack(root);
    walk_entries_in(rules, root, root, &mut stack, f)
}

fn walk_entries_in(
    rules: &IgnoreRules,
    root: &Path,
    dir: &Path,
    stack: &mut Vec<Gitignore>,
    f: &mut dyn FnMut(&Path, &Path, bool) -> Result<()>,
) -> Result<()> {
    let pushed = dir != root && rules.push_dir(dir, stack);
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let file_name = entry.file_name();
        let Some(name) = capturable_name(&file_name) else {
            continue;
        };
        let ft = entry.file_type()?;
        if ft.is_symlink() || rules.excluded(stack, &path, rel, name, ft.is_dir()) {
            continue;
        }
        if ft.is_dir() {
            walk_entries_in(rules, root, &path, stack, f)?;
            f(&path, rel, true)?;
        } else if ft.is_file() {
            f(&path, rel, false)?;
        }
    }
    if pushed {
        stack.pop();
    }
    Ok(())
}

/// Can a single path component be stored in a tree AND recreated on every
/// platform we restore on? One rule shared by snapshot and restore: a name
/// the snapshot accepts but the restore rejects would make the whole
/// snapshot unrestorable (B2). Rejects Windows-hostile names (trailing `.`
/// or space, `:`, `\`), control characters, separators and `.`/`..`.
pub fn is_portable_name(name: &str) -> bool {
    !(name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name.chars().any(char::is_control)
        || name.contains(['/', '\\', ':', '\0'])
        || Path::new(name).is_absolute())
}

/// The name of a directory entry as it would be stored in a tree, or None
/// when the entry cannot be captured (non-UTF-8 or unportable name). Paths
/// that return None are skipped by snapshots and left alone by restores.
fn capturable_name(name: &std::ffi::OsStr) -> Option<&str> {
    name.to_str().filter(|s| is_portable_name(s))
}

/// Warn once per process about each path that was left out of snapshots
/// because its name cannot be restored. Stderr only, so `re` commands
/// stay scriptable, and once only, so `re watch` does not repeat it.
fn warn_skipped(rel: &Path) {
    static WARNED: std::sync::Mutex<std::collections::BTreeSet<String>> =
        std::sync::Mutex::new(std::collections::BTreeSet::new());
    let key = rel.to_string_lossy().into_owned();
    let first = match WARNED.lock() {
        Ok(mut set) => set.insert(key.clone()),
        Err(_) => true,
    };
    if first {
        eprintln!(
            "warning: {:?} left out of snapshots: name is not restorable on every platform (trailing '.'/space, ':', '\\', control or non-UTF-8 characters)",
            key
        );
    }
}

#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Does the file at `path` carry the wanted executable bit? Always true
/// where the platform has no such bit, so restores never report a mode
/// change they cannot make.
fn exec_matches(path: &Path, exec: bool) -> bool {
    if cfg!(unix) {
        std::fs::metadata(path)
            .map(|m| is_executable(&m) == exec)
            .unwrap_or(false)
    } else {
        true
    }
}

/// Set or clear the executable bit like git does: `x` is granted wherever
/// `r` already is, so the umask the file was created with is respected.
#[cfg(unix)]
fn set_exec(path: &Path, exec: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode();
    let wanted = if exec {
        mode | ((mode & 0o444) >> 2)
    } else {
        mode & !0o111
    };
    if wanted != mode {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(wanted))
            .with_context(|| format!("setting mode of {}", path.display()))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_exec(_path: &Path, _exec: bool) -> Result<()> {
    Ok(())
}

/// Stat cache: `.causari/index/stat-cache.json`, workspace-relative path →
/// (size, mtime, blob id) of the file as it was last hashed. Recording used
/// to read and hash every file for every snapshot; with the cache a file
/// whose size and mtime are unchanged reuses its blob id without being
/// opened, so a snapshot costs one stat per file plus a read per *changed*
/// file. This is git's index trick, with git's caveat and git's fix:
///
/// * Race: a file edited twice within the mtime granularity of the
///   filesystem (one second on ext3, HFS+, FAT) and keeping its size would
///   be missed. Mitigation ("racy git"): an entry whose mtime is within
///   [`RACY_WINDOW`] of the time it was hashed is not stored at all, so the
///   file is re-read on the next snapshot, by which time any second edit
///   has moved its mtime past the window.
/// * The cache is only a cache: unreadable, missing or a different version
///   means start empty; it is rewritten atomically after each snapshot.
/// * Like git, a deliberate `touch -d` that preserves size and mtime after
///   an edit is not detected.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct StatCache {
    version: u32,
    entries: BTreeMap<String, StatEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct StatEntry {
    size: u64,
    /// Seconds and nanoseconds of the mtime since the Unix epoch; two fields
    /// because a single 128-bit integer is not portable JSON.
    mtime_s: i64,
    mtime_ns: u32,
    blob: String,
}

const STAT_CACHE_VERSION: u32 = 1;
const RACY_WINDOW: std::time::Duration = std::time::Duration::from_secs(2);

fn stat_cache_path(repo: &Repo) -> PathBuf {
    repo.dir.join("index").join("stat-cache.json")
}

fn mtime_parts(t: std::time::SystemTime) -> (i64, u32) {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_nanos()),
        Err(e) => {
            let d = e.duration();
            (-(d.as_secs() as i64), d.subsec_nanos())
        }
    }
}

impl StatCache {
    fn load(repo: &Repo) -> Self {
        let Ok(raw) = std::fs::read(stat_cache_path(repo)) else {
            return Self::default();
        };
        match serde_json::from_slice::<StatCache>(&raw) {
            Ok(c) if c.version == STAT_CACHE_VERSION => c,
            _ => Self::default(),
        }
    }

    fn save(&self, repo: &Repo) -> Result<()> {
        let path = stat_cache_path(repo);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::keys::write_atomic(&path, &serde_json::to_vec(self)?)
    }

    /// Blob id recorded for `key` if the file still has that size and mtime.
    fn lookup(&self, key: &str, size: u64, mtime: std::time::SystemTime) -> Option<&str> {
        let e = self.entries.get(key)?;
        let (s, ns) = mtime_parts(mtime);
        (e.size == size && e.mtime_s == s && e.mtime_ns == ns).then_some(e.blob.as_str())
    }
}

/// Per-snapshot working state: the cache read at the start, the cache to
/// write at the end, and the racy cut-off.
struct SnapshotCtx<'a> {
    rules: &'a IgnoreRules,
    old: &'a StatCache,
    new: BTreeMap<String, StatEntry>,
    /// Files modified at or after this instant are hashed but not cached.
    racy_after: std::time::SystemTime,
}

impl SnapshotCtx<'_> {
    fn remember(&mut self, key: String, size: u64, mtime: std::time::SystemTime, blob: &str) {
        if mtime >= self.racy_after {
            return;
        }
        let (mtime_s, mtime_ns) = mtime_parts(mtime);
        self.new.insert(
            key,
            StatEntry {
                size,
                mtime_s,
                mtime_ns,
                blob: blob.to_string(),
            },
        );
    }
}

#[cfg(test)]
thread_local! {
    /// Files opened and hashed by build_tree on this thread; lets tests
    /// prove that an unchanged tree costs zero reads.
    static FILE_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Build a tree object recursively from a directory.
/// Returns the tree id.
fn build_tree(
    store: &Store,
    ctx: &mut SnapshotCtx<'_>,
    root: &Path,
    dir: &Path,
    stack: &mut Vec<Gitignore>,
) -> Result<String> {
    let rules = ctx.rules;
    let pushed = dir != root && rules.push_dir(dir, stack);
    let mut entries = BTreeMap::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let file_name = entry.file_name();
        let name = match capturable_name(&file_name) {
            Some(s) => s.to_string(),
            None => {
                warn_skipped(rel);
                continue;
            }
        };
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            // Skip symlinks for the MVP to keep semantics simple.
            continue;
        }
        if rules.excluded(stack, &path, rel, &name, ft.is_dir()) {
            continue;
        }
        if ft.is_dir() {
            let child_id = build_tree(store, ctx, root, &path, stack)?;
            entries.insert(name, TreeEntry::tree(child_id));
        } else if ft.is_file() {
            let meta = entry
                .metadata()
                .with_context(|| format!("stat {}", path.display()))?;
            let key = rel.to_string_lossy().replace('\\', "/");
            let size = meta.len();
            let mtime = meta.modified().ok();
            let cached = mtime.and_then(|m| ctx.old.lookup(&key, size, m));
            let blob_id = match cached {
                Some(id) => id.to_string(),
                None => {
                    #[cfg(test)]
                    FILE_READS.with(|c| c.set(c.get() + 1));
                    let bytes = std::fs::read(&path)
                        .with_context(|| format!("reading {}", path.display()))?;
                    store.write_blob(&bytes)?
                }
            };
            if let Some(m) = mtime {
                ctx.remember(key, size, m, &blob_id);
            }
            entries.insert(name, TreeEntry::blob(blob_id, is_executable(&meta)));
        }
    }
    if pushed {
        stack.pop();
    }
    let tree = Tree { entries };
    store.write_tree(&tree)
}

/// Snapshot the working tree of `repo`. Returns the root tree id.
pub fn snapshot_workspace(repo: &Repo) -> Result<String> {
    let store = Store::new(repo);
    let rules = IgnoreRules::for_root(&repo.root);
    let old = StatCache::load(repo);
    let mut ctx = SnapshotCtx {
        rules: &rules,
        old: &old,
        new: BTreeMap::new(),
        racy_after: std::time::SystemTime::now() - RACY_WINDOW,
    };
    let mut stack = rules.root_stack(&repo.root);
    let tree = build_tree(&store, &mut ctx, &repo.root, &repo.root, &mut stack)?;
    if ctx.new != old.entries {
        // A cache: failing to persist it must not fail the snapshot.
        let _ = StatCache {
            version: STAT_CACHE_VERSION,
            entries: ctx.new,
        }
        .save(repo);
    }
    Ok(tree)
}

/// Restore the working tree to match the given root tree id.
/// This is the killer feature: it deletes / restores files until the
/// workspace is byte-identical to the snapshot. Ignored paths are left alone.
pub fn restore_workspace(repo: &Repo, tree_id: &str) -> Result<RestoreReport> {
    // Validate the complete object graph and destination before the first write.
    // This is preflight, not a transaction against concurrent filesystem writers.
    plan_restore(repo, tree_id)?;
    let store = Store::new(repo);
    let mut report = RestoreReport::default();
    restore_tree(&store, &repo.root, tree_id, &mut report)?;
    // After writing, walk the actual filesystem to delete files not in target.
    cleanup_extras(&store, repo, tree_id, &mut report)?;
    Ok(report)
}

/// Read-only validation and exact file counts for a quiescent workspace.
/// Reject unsupported path/type changes rather than partially applying them.
pub fn plan_restore(repo: &Repo, tree_id: &str) -> Result<RestoreReport> {
    let store = Store::new(repo);
    let mut report = RestoreReport::default();
    let mut targets = std::collections::HashSet::new();
    validate_restore_tree(&store, &repo.root, tree_id, 0, &mut targets, &mut report)?;
    // Restore walks the workspace with exactly the snapshot's eyes: whatever
    // a snapshot would skip (ignored paths, uncapturable names) a restore
    // must not delete, otherwise "sync the workspace to the snapshot" erases
    // files that were never in any snapshot.
    let rules = IgnoreRules::for_root(&repo.root);
    walk_entries(&rules, &repo.root, &mut |path, _rel, is_dir| {
        if !is_dir && !targets.contains(path) {
            report.files_deleted += 1;
        }
        Ok(())
    })?;
    Ok(report)
}

fn validate_restore_tree(
    store: &Store,
    dir: &Path,
    tree_id: &str,
    depth: usize,
    targets: &mut std::collections::HashSet<PathBuf>,
    report: &mut RestoreReport,
) -> Result<()> {
    if depth > 256 {
        bail!("snapshot exceeds supported tree depth (256)");
    }
    validate_destination(dir, true)?;
    let tree = store.read_tree(tree_id)?;
    for (name, entry) in tree.entries {
        // Validate portable single components, including Windows separators/ADS.
        if !is_portable_name(&name) || is_protected_name(&name) {
            bail!("unsafe or protected snapshot entry: {:?}", name);
        }
        let path = dir.join(&name);
        match entry.kind.as_str() {
            "tree" => validate_restore_tree(store, &path, &entry.id, depth + 1, targets, report)?,
            "blob" => {
                validate_destination(&path, false)?;
                let target = store.read_blob(&entry.id)?;
                targets.insert(path.clone());
                match std::fs::read(&path) {
                    Ok(current) if current == target && exec_matches(&path, entry.exec) => {
                        report.files_unchanged += 1
                    }
                    Ok(_) => report.files_written += 1,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => report.files_written += 1,
                    Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
                }
            }
            kind => bail!(
                "unsupported snapshot entry kind {:?} at {}",
                kind,
                path.display()
            ),
        }
    }
    Ok(())
}

fn validate_destination(path: &Path, directory: bool) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                bail!("refusing to restore through symlink: {}", path.display());
            }
            if (directory && !meta.is_dir()) || (!directory && !meta.is_file()) {
                bail!("restore path type conflict: {}", path.display());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if !directory && meta.nlink() > 1 {
                    bail!("refusing to overwrite hard-linked file: {}", path.display());
                }
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("inspecting {}", path.display())),
    }
}

#[derive(Debug, Default)]
pub struct RestoreReport {
    pub files_written: usize,
    pub files_deleted: usize,
    pub files_unchanged: usize,
}

fn restore_tree(
    store: &Store,
    dir: &Path,
    tree_id: &str,
    report: &mut RestoreReport,
) -> Result<()> {
    let tree = store.read_tree(tree_id)?;
    std::fs::create_dir_all(dir)?;
    for (name, entry) in &tree.entries {
        let path = dir.join(name);
        match entry.kind.as_str() {
            "tree" => {
                restore_tree(store, &path, &entry.id, report)?;
            }
            "blob" => {
                let target = store.read_blob(&entry.id)?;
                let same_content = match std::fs::read(&path) {
                    Ok(current) => current == target,
                    Err(_) => false,
                };
                if !same_content {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, &target)?;
                }
                if same_content && exec_matches(&path, entry.exec) {
                    report.files_unchanged += 1;
                } else {
                    set_exec(&path, entry.exec)?;
                    report.files_written += 1;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Walk the filesystem and delete any file not present in the target tree.
/// Directories the tree does not know and that are empty afterwards go too
/// (a snapshot records empty directories, so a leftover one would make the
/// restored workspace hash to a different tree); a directory that still
/// holds ignored or uncapturable files is left in place.
fn cleanup_extras(
    store: &Store,
    repo: &Repo,
    tree_id: &str,
    report: &mut RestoreReport,
) -> Result<()> {
    let mut files = std::collections::HashSet::new();
    let mut dirs = std::collections::HashSet::new();
    collect_paths(store, &PathBuf::new(), tree_id, &mut files, &mut dirs)?;

    let rules = IgnoreRules::for_root(&repo.root);
    let mut extra_files = Vec::new();
    let mut extra_dirs = Vec::new();
    walk_entries(&rules, &repo.root, &mut |path, rel, is_dir| {
        if is_dir {
            if !dirs.contains(rel) {
                extra_dirs.push(path.to_path_buf());
            }
        } else if !files.contains(rel) {
            extra_files.push(path.to_path_buf());
        }
        Ok(())
    })?;
    for path in extra_files {
        std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        report.files_deleted += 1;
    }
    // Post-order from the walk: children before parents. Non-empty is not
    // an error here, it means the directory still holds files we must keep.
    for path in extra_dirs {
        let _ = std::fs::remove_dir(&path);
    }
    Ok(())
}

fn collect_paths(
    store: &Store,
    prefix: &Path,
    tree_id: &str,
    files: &mut std::collections::HashSet<PathBuf>,
    dirs: &mut std::collections::HashSet<PathBuf>,
) -> Result<()> {
    let tree = store.read_tree(tree_id)?;
    for (name, entry) in &tree.entries {
        let p = prefix.join(name);
        match entry.kind.as_str() {
            "blob" => {
                files.insert(p);
            }
            "tree" => {
                collect_paths(store, &p, &entry.id, files, dirs)?;
                dirs.insert(p);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Compute a flat map of relative path -> blob id for a given tree.
/// Useful for diffing two snapshots.
pub fn flatten_tree(store: &Store, tree_id: &str) -> Result<BTreeMap<PathBuf, String>> {
    let mut out = BTreeMap::new();
    flatten_inner(store, &PathBuf::new(), tree_id, &mut out)?;
    Ok(out)
}

fn flatten_inner(
    store: &Store,
    prefix: &Path,
    tree_id: &str,
    out: &mut BTreeMap<PathBuf, String>,
) -> Result<()> {
    let tree = store.read_tree(tree_id)?;
    for (name, entry) in &tree.entries {
        let p = prefix.join(name);
        match entry.kind.as_str() {
            "blob" => {
                out.insert(p, entry.id.clone());
            }
            "tree" => {
                flatten_inner(store, &p, &entry.id, out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Effective reads of an event = files declared by the agent in `reads`
/// PLUS every file the event modified (because writing a file implies reading
/// its previous contents). Returned as a deduped vector of PathBufs.
pub fn effective_reads(store: &Store, ev: &crate::object::Event) -> Result<Vec<PathBuf>> {
    let mut set: std::collections::HashSet<PathBuf> = ev
        .reads
        .iter()
        .map(|s| PathBuf::from(s.replace('\\', "/")))
        .collect();
    let writes = effective_writes(store, &ev.pre_snapshot, &ev.post_snapshot)?;
    for w in writes {
        set.insert(w);
    }
    Ok(set.into_iter().collect())
}

/// Collect the lines INSERTED between two snapshots, across all changed
/// files, capped at `cap` lines. This is the input to the capture layer's
/// correlation engine: inserted lines are searched inside recent LLM
/// completions to attribute the change to the prompt that caused it.
pub fn added_lines_between(
    store: &Store,
    pre_snapshot_id: &str,
    post_snapshot_id: &str,
    cap: usize,
) -> Result<Vec<String>> {
    use similar::{ChangeTag, TextDiff};

    let pre_snap = store.read_snapshot(pre_snapshot_id)?;
    let post_snap = store.read_snapshot(post_snapshot_id)?;
    let pre = flatten_tree(store, &pre_snap.tree)?;
    let post = flatten_tree(store, &post_snap.tree)?;

    let mut out = Vec::new();
    for (path, post_id) in &post {
        if out.len() >= cap {
            break;
        }
        let pre_id = pre.get(path);
        if pre_id == Some(post_id) {
            continue;
        }
        let post_text = String::from_utf8(store.read_blob(post_id)?).unwrap_or_default();
        let pre_text = match pre_id {
            Some(id) => String::from_utf8(store.read_blob(id)?).unwrap_or_default(),
            None => String::new(),
        };
        let diff = TextDiff::from_lines(&pre_text, &post_text);
        for change in diff.iter_all_changes() {
            if change.tag() == ChangeTag::Insert {
                out.push(change.value().trim_end_matches('\n').to_string());
                if out.len() >= cap {
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// Compute the set of files that *actually changed* between the pre and post
/// snapshots of an event (additions, deletions, modifications).
///
/// This is the ground truth for "what the agent wrote", independent of what
/// the agent claimed in its `writes` field. Causari trusts the filesystem.
pub fn effective_writes(
    store: &Store,
    pre_snapshot_id: &str,
    post_snapshot_id: &str,
) -> Result<Vec<PathBuf>> {
    let pre_snap = store.read_snapshot(pre_snapshot_id)?;
    let post_snap = store.read_snapshot(post_snapshot_id)?;
    let pre = flatten_tree(store, &pre_snap.tree)?;
    let post = flatten_tree(store, &post_snap.tree)?;

    let mut changed: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    for (path, blob_id) in &post {
        match pre.get(path) {
            Some(pre_id) if pre_id == blob_id => {} // unchanged
            _ => {
                changed.insert(path.clone());
            }
        }
    }
    for path in pre.keys() {
        if !post.contains_key(path) {
            changed.insert(path.clone()); // deletion counts
        }
    }
    Ok(changed.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{Event, Snapshot};

    fn test_repo() -> (tempfile::TempDir, Repo) {
        let tmp = tempfile::tempdir().unwrap();
        let repo = Repo::init(tmp.path()).unwrap();
        (tmp, repo)
    }

    fn write(repo: &Repo, rel: &str, content: &str) {
        let p = repo.root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content).unwrap();
    }

    fn snap(repo: &Repo, store: &Store) -> String {
        let tree = snapshot_workspace(repo).unwrap();
        store
            .write_snapshot(&Snapshot {
                tree,
                created_at: "2026-01-01T00:00:00Z".into(),
            })
            .unwrap()
    }

    #[test]
    fn stress_restore_matches_24_distinct_snapshots_with_binary_files() {
        let (_tmp, repo) = test_repo();
        for round in 0u32..24 {
            for file in 0u32..48 {
                let data: Vec<u8> = (0u32..257)
                    .map(|n| ((n * 37 + file * 13 + round * 7) % 256) as u8)
                    .collect();
                let path = repo.root.join(format!("group{}/file{file}", file % 4));
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, data).unwrap();
            }
            let target = snapshot_workspace(&repo).unwrap();
            write(&repo, "group0/file0", "modified");
            std::fs::remove_file(repo.root.join("group1/file1")).unwrap();
            write(&repo, "extra", "delete this");
            write(&repo, ".env", "must survive");
            let before = snapshot_workspace(&repo).unwrap();
            let plan = plan_restore(&repo, &target).unwrap();
            assert_eq!(snapshot_workspace(&repo).unwrap(), before);
            assert_eq!(
                (plan.files_written, plan.files_deleted, plan.files_unchanged),
                (2, 1, 46)
            );
            restore_workspace(&repo, &target).unwrap();
            assert_eq!(snapshot_workspace(&repo).unwrap(), target);
            assert_eq!(
                std::fs::read_to_string(repo.root.join(".env")).unwrap(),
                "must survive"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn hard_link_destination_is_rejected() {
        let (_tmp, repo) = test_repo();
        write(&repo, "file", "original");
        let tree = snapshot_workspace(&repo).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::hard_link(repo.root.join("file"), outside.path().join("alias")).unwrap();
        write(&repo, "file", "external state");
        assert!(restore_workspace(&repo, &tree).is_err());
        assert_eq!(
            std::fs::read_to_string(outside.path().join("alias")).unwrap(),
            "external state"
        );
    }

    #[test]
    fn path_type_conflict_does_not_partially_restore() {
        let (_tmp, repo) = test_repo();
        write(&repo, "a", "original");
        write(&repo, "z", "file");
        let tree = snapshot_workspace(&repo).unwrap();
        write(&repo, "a", "new work");
        std::fs::remove_file(repo.root.join("z")).unwrap();
        write(&repo, "z/child", "preserve");
        assert!(restore_workspace(&repo, &tree).is_err());
        assert_eq!(
            std::fs::read_to_string(repo.root.join("a")).unwrap(),
            "new work"
        );
    }

    #[test]
    fn missing_late_blob_does_not_partially_restore() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        write(&repo, "a.txt", "before");
        write(&repo, "z.txt", "late");
        let tree = snapshot_workspace(&repo).unwrap();
        let flat = flatten_tree(&store, &tree).unwrap();
        let id = &flat[Path::new("z.txt")];
        std::fs::remove_file(repo.objects_dir().join(&id[..2]).join(&id[2..])).unwrap();
        write(&repo, "a.txt", "valuable new work");
        assert!(restore_workspace(&repo, &tree).is_err());
        assert_eq!(
            std::fs::read_to_string(repo.root.join("a.txt")).unwrap(),
            "valuable new work"
        );
    }

    #[test]
    fn unsafe_tree_names_and_unknown_kinds_are_rejected() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        let blob = store.write_blob(b"bad").unwrap();
        for name in [
            "../escape",
            "/absolute",
            "a/b",
            "a\\b",
            ".causari",
            ".env",
            "..",
            "",
        ] {
            let tree = store
                .write_tree(&Tree {
                    entries: BTreeMap::from([(name.into(), TreeEntry::blob(blob.clone(), false))]),
                })
                .unwrap();
            assert!(
                restore_workspace(&repo, &tree).is_err(),
                "accepted {name:?}"
            );
        }
        let tree = store
            .write_tree(&Tree {
                entries: BTreeMap::from([(
                    "safe".into(),
                    TreeEntry {
                        kind: "unknown".into(),
                        id: blob,
                        exec: false,
                    },
                )]),
            })
            .unwrap();
        assert!(restore_workspace(&repo, &tree).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn restore_refuses_symlink_escape_without_touching_external_file() {
        let (_tmp, repo) = test_repo();
        let outside = tempfile::tempdir().unwrap();
        write(&repo, "dir/file", "snapshot");
        let tree = snapshot_workspace(&repo).unwrap();
        std::fs::remove_dir_all(repo.root.join("dir")).unwrap();
        std::fs::write(outside.path().join("file"), "external").unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.root.join("dir")).unwrap();
        assert!(restore_workspace(&repo, &tree).is_err());
        assert_eq!(
            std::fs::read_to_string(outside.path().join("file")).unwrap(),
            "external"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unportable_names_are_skipped_at_snapshot_time_so_snapshots_stay_restorable() {
        // Regression (B2): build_tree accepted any name the filesystem
        // allowed, validate_restore_tree rejected several of them, and one
        // such file disabled revert/bisect/switch/fork for every snapshot.
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        write(&repo, "good.rs", "fn main() {}");
        write(&repo, "dir/also good", "x");
        for bad in [
            "trailing.",
            "trailing ",
            "colon:name",
            "back\\slash",
            "ctl\u{1}char",
        ] {
            write(&repo, bad, "unportable");
            write(&repo, &format!("dir/{bad}"), "unportable");
        }
        std::fs::create_dir(repo.root.join("baddir.")).unwrap();
        write(&repo, "baddir./inside", "unportable dir");

        let tree = snapshot_workspace(&repo).unwrap();
        let mut paths: Vec<String> = flatten_tree(&store, &tree)
            .unwrap()
            .keys()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        paths.sort();
        assert_eq!(paths, vec!["dir/also good", "good.rs"]);

        // The snapshot is restorable, and the restore leaves the skipped
        // files alone: they were never captured, so deleting them would be
        // data loss.
        write(&repo, "good.rs", "changed");
        let plan = plan_restore(&repo, &tree).unwrap();
        assert_eq!(
            (plan.files_written, plan.files_deleted, plan.files_unchanged),
            (1, 0, 1)
        );
        restore_workspace(&repo, &tree).unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.root.join("trailing.")).unwrap(),
            "unportable"
        );
        assert_eq!(
            std::fs::read_to_string(repo.root.join("baddir./inside")).unwrap(),
            "unportable dir"
        );
        assert_eq!(snapshot_workspace(&repo).unwrap(), tree);
    }

    #[test]
    fn tree_ids_are_unchanged_when_no_executable_bit_is_set() {
        // The exec field is absent from the canonical JSON when false, so
        // every tree written by an older binary keeps its id.
        let entries = BTreeMap::from([(
            "main.rs".to_string(),
            TreeEntry::blob("ab".repeat(32), false),
        )]);
        let json = crate::object::canonical_json(&Tree { entries }).unwrap();
        assert_eq!(
            String::from_utf8(json).unwrap(),
            format!(
                r#"{{"entries":{{"main.rs":{{"id":"{}","kind":"blob"}}}}}}"#,
                "ab".repeat(32)
            )
        );
        let with_exec =
            BTreeMap::from([("run.sh".to_string(), TreeEntry::blob("cd".repeat(32), true))]);
        let json = crate::object::canonical_json(&Tree { entries: with_exec }).unwrap();
        assert!(String::from_utf8(json).unwrap().contains(r#""exec":true"#));

        // Old trees without the field deserialize with exec = false.
        let old: Tree =
            serde_json::from_str(r#"{"entries":{"a":{"id":"x","kind":"blob"}}}"#).unwrap();
        assert!(!old.entries["a"].exec);
    }

    #[cfg(unix)]
    #[test]
    fn executable_bit_is_captured_and_restored() {
        use std::os::unix::fs::PermissionsExt;
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        write(&repo, "run.sh", "#!/bin/sh\necho hi\n");
        write(&repo, "lib.rs", "pub fn f() {}");
        let sh = repo.root.join("run.sh");
        std::fs::set_permissions(&sh, std::fs::Permissions::from_mode(0o755)).unwrap();

        let tree = snapshot_workspace(&repo).unwrap();
        let root = store.read_tree(&tree).unwrap();
        assert!(root.entries["run.sh"].exec);
        assert!(!root.entries["lib.rs"].exec);

        // Flipping only the mode is a change the snapshot sees …
        std::fs::set_permissions(&sh, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_ne!(snapshot_workspace(&repo).unwrap(), tree);

        // … and the restore repairs, counting it as a write in both the plan
        // and the actual restore.
        let plan = plan_restore(&repo, &tree).unwrap();
        assert_eq!((plan.files_written, plan.files_unchanged), (1, 1));
        let report = restore_workspace(&repo, &tree).unwrap();
        assert_eq!((report.files_written, report.files_unchanged), (1, 1));
        assert_ne!(
            std::fs::metadata(&sh).unwrap().permissions().mode() & 0o111,
            0
        );
        assert_eq!(snapshot_workspace(&repo).unwrap(), tree);

        // A non-executable entry clears a stray bit, and a deleted executable
        // comes back executable.
        std::fs::set_permissions(
            repo.root.join("lib.rs"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        std::fs::remove_file(&sh).unwrap();
        restore_workspace(&repo, &tree).unwrap();
        assert_eq!(
            std::fs::metadata(repo.root.join("lib.rs"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
        assert_ne!(
            std::fs::metadata(&sh).unwrap().permissions().mode() & 0o111,
            0
        );
        assert_eq!(snapshot_workspace(&repo).unwrap(), tree);
    }

    #[test]
    fn ignored_paths_never_enter_snapshots() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        write(&repo, "src/main.rs", "fn main() {}");
        write(&repo, "node_modules/pkg/index.js", "x");
        write(&repo, "target/debug/bin", "x");

        let tree_id = snapshot_workspace(&repo).unwrap();
        let flat = flatten_tree(&store, &tree_id).unwrap();
        let paths: Vec<String> = flat
            .keys()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(paths, vec!["src/main.rs"]);
    }

    fn paths_of(store: &Store, tree: &str) -> Vec<String> {
        flatten_tree(store, tree)
            .unwrap()
            .keys()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn git_work_tree_follows_gitignore_and_keeps_nested_build_dirs() {
        // Regression (B3): the built-in list used to match any path
        // component, silently dropping src/build/gen.rs and packages/dist/.
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        std::fs::create_dir_all(repo.root.join(".git/info")).unwrap();
        write(&repo, ".git/config", "[core]");
        write(&repo, ".git/info/exclude", "scratch/\n");
        write(&repo, ".gitignore", "*.log\n/generated/\n");
        write(&repo, "src/main.rs", "fn main() {}");
        write(&repo, "src/build/gen.rs", "// generated but tracked");
        write(&repo, "packages/dist/index.js", "tracked bundle");
        write(&repo, "node_modules/x.js", "dep");
        write(&repo, "target/debug/bin", "build output");
        write(&repo, "debug.log", "noise");
        write(&repo, "src/deep/trace.log", "noise");
        write(&repo, "generated/out.rs", "noise");
        write(&repo, "scratch/notes", "excluded via info/exclude");
        write(&repo, "sub/.gitignore", "local.txt\n!keep.log\n");
        write(&repo, "sub/local.txt", "ignored by nested file");
        write(&repo, "sub/keep.log", "re-included by nested file");
        write(&repo, "sub/other.log", "still ignored by root file");
        write(&repo, ".env", "SECRET=1");

        let tree = snapshot_workspace(&repo).unwrap();
        assert_eq!(
            paths_of(&store, &tree),
            vec![
                ".gitignore",
                "packages/dist/index.js",
                "src/build/gen.rs",
                "src/main.rs",
                "sub/.gitignore",
                "sub/keep.log",
            ]
        );
    }

    #[test]
    fn gitignore_can_re_include_a_built_in_top_level_directory() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        std::fs::create_dir_all(repo.root.join(".git")).unwrap();
        write(&repo, ".gitignore", "!/dist/\n");
        write(&repo, "dist/bundle.js", "tracked build artefact");
        write(&repo, "build/out", "still excluded");
        let tree = snapshot_workspace(&repo).unwrap();
        assert_eq!(
            paths_of(&store, &tree),
            vec![".gitignore", "dist/bundle.js"]
        );
    }

    #[test]
    fn restore_leaves_gitignored_files_alone() {
        // A restore syncs the workspace to the snapshot; files the snapshot
        // never captured because .gitignore excluded them are not "extras".
        let (_tmp, repo) = test_repo();
        std::fs::create_dir_all(repo.root.join(".git")).unwrap();
        write(&repo, ".gitignore", "*.log\n");
        write(&repo, "a.txt", "one");
        let tree = snapshot_workspace(&repo).unwrap();

        write(&repo, "a.txt", "two");
        write(&repo, "extra.txt", "delete me");
        write(&repo, "session.log", "keep me");
        write(&repo, "src/build/gen.rs", "delete me too");

        let plan = plan_restore(&repo, &tree).unwrap();
        assert_eq!((plan.files_written, plan.files_deleted), (1, 2));
        let report = restore_workspace(&repo, &tree).unwrap();
        assert_eq!((report.files_written, report.files_deleted), (1, 2));
        assert!(!repo.root.join("extra.txt").exists());
        // The directories that only existed for the extra file go too;
        // a directory still holding an ignored file stays.
        assert!(!repo.root.join("src").exists());
        assert_eq!(
            std::fs::read_to_string(repo.root.join("session.log")).unwrap(),
            "keep me"
        );
        assert_eq!(snapshot_workspace(&repo).unwrap(), tree);

        write(&repo, "logs/run.log", "ignored, keeps its dir alive");
        restore_workspace(&repo, &tree).unwrap();
        assert!(repo.root.join("logs/run.log").exists());
    }

    #[test]
    fn without_git_the_built_in_list_is_the_whole_rule() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);
        write(&repo, ".gitignore", "*.log\n");
        write(&repo, "debug.log", "no git, so .gitignore is not consulted");
        write(&repo, "src/main.rs", "x");
        write(&repo, "src/build/gen.rs", "dropped: build dir at any depth");
        write(&repo, "src/.idea/ws.xml", "kept: .idea only at top level");
        write(&repo, ".idea/ws.xml", "dropped");
        let tree = snapshot_workspace(&repo).unwrap();
        assert_eq!(
            paths_of(&store, &tree),
            vec![".gitignore", "debug.log", "src/.idea/ws.xml", "src/main.rs"]
        );
    }

    #[test]
    fn is_ignored_is_the_cheap_rule() {
        for yes in [
            ".causari/HEAD",
            "a/.git/config",
            ".env",
            "cfg/.env.local",
            "dist",
            "target/debug/x",
            "src/dist/x.js",
            "pkg/node_modules/a/b.js",
            ".vscode/settings.json",
        ] {
            assert!(is_ignored(Path::new(yes)), "{yes}");
        }
        for no in [
            "src/main.rs",
            "src/.vscode/x",
            "environment",
            "a/.envrc",
            "",
        ] {
            assert!(!is_ignored(Path::new(no)), "{no}");
        }
    }

    fn reads_during<T>(f: impl FnOnce() -> T) -> usize {
        let before = FILE_READS.with(|c| c.get());
        let _ = f();
        FILE_READS.with(|c| c.get()) - before
    }

    fn age(path: &Path, secs: u64) {
        let old = std::time::SystemTime::now() - Duration::from_secs(secs);
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }

    fn cache_entries(repo: &Repo) -> BTreeMap<String, StatEntry> {
        StatCache::load(repo).entries
    }

    use std::time::Duration;

    #[test]
    fn stat_cache_makes_snapshots_cost_only_the_changed_files() {
        let (_tmp, repo) = test_repo();
        for i in 0..200 {
            let rel = format!("d{}/f{i}.txt", i % 10);
            write(&repo, &rel, &format!("content {i}"));
            age(&repo.root.join(&rel), 10);
        }

        let mut first = None;
        let reads = reads_during(|| first = Some(snapshot_workspace(&repo).unwrap()));
        assert_eq!(reads, 200);
        let before = cache_entries(&repo);
        assert_eq!(before.len(), 200);

        // Unchanged tree: not a single file is opened, same tree id.
        let mut second = None;
        let reads = reads_during(|| second = Some(snapshot_workspace(&repo).unwrap()));
        assert_eq!(reads, 0);
        assert_eq!(first, second);

        // One edit (different size): exactly one read, one cache entry moves.
        write(&repo, "d3/f13.txt", "edited content, longer");
        age(&repo.root.join("d3/f13.txt"), 5);
        let mut third = None;
        let reads = reads_during(|| third = Some(snapshot_workspace(&repo).unwrap()));
        assert_eq!(reads, 1);
        assert_ne!(third, second);
        let after = cache_entries(&repo);
        assert_eq!(after.len(), 200);
        let changed: Vec<&String> = after
            .iter()
            .filter(|(k, v)| before.get(*k) != Some(v))
            .map(|(k, _)| k)
            .collect();
        assert_eq!(changed, vec!["d3/f13.txt"]);

        // Same size, new mtime: still detected through the mtime.
        write(&repo, "d3/f13.txt", "edited content, LONGER");
        age(&repo.root.join("d3/f13.txt"), 4);
        let mut fourth = None;
        let reads = reads_during(|| fourth = Some(snapshot_workspace(&repo).unwrap()));
        assert_eq!(reads, 1);
        assert_ne!(fourth, third);

        // A deleted file leaves the cache.
        std::fs::remove_file(repo.root.join("d0/f0.txt")).unwrap();
        snapshot_workspace(&repo).unwrap();
        assert_eq!(cache_entries(&repo).len(), 199);
    }

    #[test]
    fn freshly_modified_files_are_not_cached() {
        // Racy-git rule: an mtime within RACY_WINDOW of the hash time is
        // not trusted, so a second edit inside the filesystem's mtime
        // granularity cannot be missed.
        let (_tmp, repo) = test_repo();
        write(&repo, "hot.txt", "just written");
        assert_eq!(reads_during(|| snapshot_workspace(&repo).unwrap()), 1);
        assert!(!cache_entries(&repo).contains_key("hot.txt"));
        assert_eq!(reads_during(|| snapshot_workspace(&repo).unwrap()), 1);

        age(&repo.root.join("hot.txt"), 10);
        assert_eq!(reads_during(|| snapshot_workspace(&repo).unwrap()), 1);
        assert!(cache_entries(&repo).contains_key("hot.txt"));
        assert_eq!(reads_during(|| snapshot_workspace(&repo).unwrap()), 0);
    }

    #[test]
    fn corrupt_or_foreign_stat_cache_is_ignored_and_rebuilt() {
        let (_tmp, repo) = test_repo();
        write(&repo, "a.txt", "a");
        age(&repo.root.join("a.txt"), 10);
        let tree = snapshot_workspace(&repo).unwrap();

        for junk in ["", "{", "[1,2]", r#"{"version":99,"entries":{}}"#] {
            std::fs::write(stat_cache_path(&repo), junk).unwrap();
            assert_eq!(
                reads_during(|| assert_eq!(snapshot_workspace(&repo).unwrap(), tree)),
                1
            );
            let rebuilt = StatCache::load(&repo);
            assert_eq!(rebuilt.version, STAT_CACHE_VERSION);
            assert_eq!(rebuilt.entries.len(), 1);
        }
        // A tree written from the cache is byte-identical to one hashed
        // from scratch: the cache only ever short-circuits the read.
        std::fs::remove_file(stat_cache_path(&repo)).unwrap();
        assert_eq!(snapshot_workspace(&repo).unwrap(), tree);
    }

    #[test]
    fn dotenv_secrets_never_enter_snapshots() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        write(&repo, "src/main.rs", "fn main() {}");
        write(&repo, ".env", "OPENAI_API_KEY=sk-secret");
        write(&repo, ".env.production", "DB_URL=postgres://secret");
        write(&repo, "config/.env.local", "TOKEN=nope");

        let tree_id = snapshot_workspace(&repo).unwrap();
        let flat = flatten_tree(&store, &tree_id).unwrap();
        let paths: Vec<String> = flat
            .keys()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        // Only the source file is captured; every dotenv variant is excluded.
        assert_eq!(paths, vec!["src/main.rs"]);
    }

    #[test]
    fn snapshot_restore_roundtrip() {
        let (_tmp, repo) = test_repo();

        write(&repo, "a.txt", "original A");
        write(&repo, "dir/b.txt", "original B");
        let tree_before = snapshot_workspace(&repo).unwrap();

        // Mutate the workspace: edit, delete, add.
        write(&repo, "a.txt", "EDITED");
        std::fs::remove_file(repo.root.join("dir/b.txt")).unwrap();
        write(&repo, "new.txt", "added later");

        let report = restore_workspace(&repo, &tree_before).unwrap();
        assert_eq!(report.files_written, 2); // a.txt restored, dir/b.txt recreated
        assert_eq!(report.files_deleted, 1); // new.txt removed

        assert_eq!(
            std::fs::read_to_string(repo.root.join("a.txt")).unwrap(),
            "original A"
        );
        assert_eq!(
            std::fs::read_to_string(repo.root.join("dir/b.txt")).unwrap(),
            "original B"
        );
        assert!(!repo.root.join("new.txt").exists());

        // Restored workspace must hash to the exact same tree.
        assert_eq!(snapshot_workspace(&repo).unwrap(), tree_before);
    }

    #[test]
    fn effective_writes_sees_adds_edits_and_deletes() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        write(&repo, "keep.txt", "same");
        write(&repo, "edit.txt", "v1");
        write(&repo, "gone.txt", "bye");
        let pre = snap(&repo, &store);

        write(&repo, "edit.txt", "v2");
        std::fs::remove_file(repo.root.join("gone.txt")).unwrap();
        write(&repo, "fresh.txt", "hi");
        let post = snap(&repo, &store);

        let changed: Vec<String> = effective_writes(&store, &pre, &post)
            .unwrap()
            .into_iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(changed, vec!["edit.txt", "fresh.txt", "gone.txt"]);
    }

    #[test]
    fn added_lines_between_returns_only_insertions_and_respects_cap() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        write(&repo, "f.txt", "one\ntwo\n");
        let pre = snap(&repo, &store);
        write(&repo, "f.txt", "one\ntwo\nthree\nfour\n");
        let post = snap(&repo, &store);

        let added = added_lines_between(&store, &pre, &post, 100).unwrap();
        assert_eq!(added, vec!["three", "four"]);

        let capped = added_lines_between(&store, &pre, &post, 1).unwrap();
        assert_eq!(capped.len(), 1);
    }

    #[test]
    fn effective_reads_include_modified_files() {
        let (_tmp, repo) = test_repo();
        let store = Store::new(&repo);

        write(&repo, "w.txt", "v1");
        let pre = snap(&repo, &store);
        write(&repo, "w.txt", "v2");
        let post = snap(&repo, &store);

        let ev = Event {
            schema: "causari.event.v0.2".into(),
            parent: None,
            agent: None,
            model: None,
            tool: None,
            message: None,
            prompt: None,
            reasoning: None,
            reads: vec!["ctx.txt".into()],
            writes: vec![],
            tokens_in: None,
            tokens_out: None,
            cost_usd: None,
            pre_snapshot: pre,
            post_snapshot: post,
            exit_code: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            evidence: None,
        };
        let mut reads: Vec<String> = effective_reads(&store, &ev)
            .unwrap()
            .into_iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        reads.sort();
        // Declared read + the file the event modified (writing implies reading).
        assert_eq!(reads, vec!["ctx.txt", "w.txt"]);
    }
}
