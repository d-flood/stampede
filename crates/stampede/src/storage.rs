use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use sha2::{Digest, Sha256};

use crate::{config, git};

const GIB: u64 = 1024 * 1024 * 1024;
pub const ENVIRONMENT: [&str; 9] = ["TMPDIR", "TMP", "TEMP", "CLAUDE_CODE_TMPDIR", "NODE_COMPILE_CACHE", "npm_config_cache", "CARGO_TARGET_DIR", "STAMPEDE_SCRATCH", "STAMPEDE_CACHE"];

#[derive(Clone)]
pub struct Storage {
    data: PathBuf,
    root: PathBuf,
}

pub struct Session {
    storage: Storage,
    owner: String,
    scratch: PathBuf,
    cache: PathBuf,
    _scratch_lease: File,
    _cache_lease: File,
    pub env: BTreeMap<String, String>,
}

fn lease(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path.join(".lease"))
}

impl Storage {
    pub fn new(data: &Path) -> Self {
        Self { data: data.to_owned(), root: data.join("storage") }
    }

    pub async fn prepare(&self, owner: &str, cwd: &Path, overrides: &BTreeMap<String, String>) -> io::Result<Session> {
        let repository = git::common_dir(cwd).await;
        let identity = repository.as_deref().unwrap_or(cwd);
        let key: String = Sha256::digest(identity.as_os_str().as_encoded_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
        fs::create_dir_all(&self.root)?;
        let maintenance = lease(&self.root)?;
        maintenance.lock_shared()?;
        let scratch = self.root.join("scratch").join(owner);
        let cache = self.root.join("cache").join(key);
        fs::create_dir_all(&scratch)?;
        fs::create_dir_all(&cache)?;
        let scratch = fs::canonicalize(scratch)?;
        let cache = fs::canonicalize(cache)?;
        let scratch_lease = lease(&scratch)?;
        scratch_lease.lock_shared()?;
        let cache_lease = lease(&cache)?;
        cache_lease.lock_shared()?;
        fs::write(scratch.join(".cache"), cache.as_os_str().as_encoded_bytes())?;
        fs::write(cache.join(".used"), b"")?;
        let temp = alias(&scratch).unwrap_or_else(|error| {
            eprintln!("cannot alias temporary storage for {owner}: {error}");
            scratch.clone()
        });
        let mut env = BTreeMap::from([
            ("TMPDIR".to_owned(), temp.to_string_lossy().into_owned()),
            ("TMP".to_owned(), temp.to_string_lossy().into_owned()),
            ("TEMP".to_owned(), temp.to_string_lossy().into_owned()),
            ("CLAUDE_CODE_TMPDIR".to_owned(), scratch.to_string_lossy().into_owned()),
            ("NODE_COMPILE_CACHE".to_owned(), cache.join("node").to_string_lossy().into_owned()),
            ("npm_config_cache".to_owned(), cache.join("npm").to_string_lossy().into_owned()),
        ]);
        let checkout = repository.as_deref().and_then(Path::parent).unwrap_or(cwd);
        env.insert("CARGO_TARGET_DIR".to_owned(), checkout.join("target").to_string_lossy().into_owned());
        env.extend(overrides.clone());
        env.insert("STAMPEDE_SCRATCH".to_owned(), scratch.to_string_lossy().into_owned());
        env.insert("STAMPEDE_CACHE".to_owned(), cache.to_string_lossy().into_owned());
        Ok(Session { storage: self.clone(), owner: owner.to_owned(), scratch, cache, _scratch_lease: scratch_lease, _cache_lease: cache_lease, env })
    }

    fn owned(&self, owner: &str) -> bool {
        self.data.join("chats").join(format!("{owner}.json")).exists()
    }

    fn remove_scratch(&self, path: &Path) -> io::Result<()> {
        let Some(owner) = path.file_name().and_then(|name| name.to_str()) else { return Ok(()) };
        if self.owned(owner) || active_storage().contains(path) { return Ok(()) }
        let file = match lease(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            file => file?,
        };
        if file.try_lock().is_ok() {
            match fs::remove_dir_all(path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                result => result?,
            }
            if let Ok(link) = alias_path(path) { let _ = fs::remove_file(link); }
        }
        Ok(())
    }

    pub fn maintain(&self, settings: &config::Storage) -> io::Result<()> {
        if !self.root.exists() { return Ok(()) }
        let maintenance = lease(&self.root)?;
        if maintenance.try_lock().is_err() { return Ok(()) }
        let mut active = active_storage();
        for path in directories(&self.root.join("scratch"))? {
            if active.contains(&path) { continue }
            if path.file_name().and_then(|name| name.to_str()).is_some_and(|owner| self.owned(owner)) {
                if let Ok(cache) = fs::read_to_string(path.join(".cache")) { active.insert(PathBuf::from(cache)); }
            } else {
                self.remove_scratch(&path)?;
            }
        }
        self.trim(settings.cache_max_gib.saturating_mul(GIB), Duration::from_secs(settings.cache_max_age_days.saturating_mul(86400)), &active)
    }

    fn trim(&self, limit: u64, age: Duration, active: &HashSet<PathBuf>) -> io::Result<()> {
        let mut entries = Vec::new();
        let mut total = 0u64;
        for path in directories(&self.root.join("cache"))? {
            let bytes = size(&path)?;
            total = total.saturating_add(bytes);
            let used = fs::metadata(path.join(".used")).and_then(|metadata| metadata.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push((used, path, bytes));
        }
        entries.sort_by_key(|(used, _, _)| *used);
        for (used, path, bytes) in entries {
            if active.contains(&path) { continue }
            if total <= limit && used.elapsed().unwrap_or_default() < age { continue }
            let file = lease(&path)?;
            if file.try_lock().is_err() { continue }
            fs::remove_dir_all(path)?;
            total = total.saturating_sub(bytes);
        }
        Ok(())
    }

    pub fn pressure(&self, project: &Path, min_free_gib: u64) -> io::Result<Option<String>> {
        let required = min_free_gib.saturating_mul(GIB);
        if required == 0 { return Ok(None) }
        for path in [&self.data, project] {
            let filesystem = nix::sys::statvfs::statvfs(path).map_err(io::Error::from)?;
            let available = filesystem.blocks_available().saturating_mul(filesystem.fragment_size());
            if available >= required { continue }
            let mut consumers = Vec::new();
            for candidate in [self.root.join("scratch"), self.root.join("cache"), self.data.join("worktrees"), project.join("target")] {
                consumers.push((size(&candidate)?, candidate));
            }
            consumers.sort_by_key(|(bytes, _)| std::cmp::Reverse(*bytes));
            let largest = consumers.into_iter().filter(|(bytes, _)| *bytes > 0).take(3)
                .map(|(bytes, path)| format!("{} ({:.2} GiB)", path.display(), bytes as f64 / GIB as f64)).collect::<Vec<_>>().join(", ");
            let mut message = format!("Low disk space at {}: {:.2} GiB available; {min_free_gib} GiB required.", path.display(), available as f64 / GIB as f64);
            if !largest.is_empty() { message.push_str(&format!(" Largest consumers: {largest}.")); }
            return Ok(Some(message));
        }
        Ok(None)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = fs::write(self.cache.join(".used"), b"");
        let _ = self._scratch_lease.unlock();
        let _ = self._cache_lease.unlock();
        if let Err(error) = self.storage.remove_scratch(&self.scratch) {
            if error.kind() != io::ErrorKind::NotFound { eprintln!("cannot clean temporary storage for {}: {error}", self.owner); }
        }
    }
}

pub fn inherited_leases() -> io::Result<Vec<File>> {
    ["STAMPEDE_SCRATCH", "STAMPEDE_CACHE"].into_iter().filter_map(std::env::var_os).map(|path| {
        let file = lease(Path::new(&path))?;
        file.lock_shared()?;
        Ok(file)
    }).collect()
}

fn alias_path(scratch: &Path) -> io::Result<PathBuf> {
    let uid = fs::metadata("/proc/self")?.uid();
    let name: String = Sha256::digest(scratch.as_os_str().as_encoded_bytes()).iter().take(8).map(|byte| format!("{byte:02x}")).collect();
    Ok(PathBuf::from(format!("/tmp/stampede-{uid}")).join(name))
}

/// Links a short path to `scratch` for TMPDIR, since Chrome and others bind Unix sockets under TMPDIR and a socket
/// path must fit in 108 bytes.
fn alias(scratch: &Path) -> io::Result<PathBuf> {
    let link = alias_path(scratch)?;
    let base = link.parent().expect("an alias has a parent");
    match fs::DirBuilder::new().mode(0o700).create(base) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {},
        result => result?,
    }
    let metadata = fs::symlink_metadata(base)?;
    if !metadata.is_dir() || metadata.uid() != fs::metadata("/proc/self")?.uid() || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::other(format!("{} is not a private directory", base.display())));
    }
    match std::os::unix::fs::symlink(scratch, &link) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && fs::read_link(&link)? == scratch => {},
        result => result?,
    }
    Ok(link)
}

fn active_storage() -> HashSet<PathBuf> {
    let mut paths = HashSet::new();
    let Ok(entries) = fs::read_dir("/proc") else { return paths };
    for process in entries.flatten() {
        if process.file_name().to_str().is_none_or(|name| !name.bytes().all(|byte| byte.is_ascii_digit())) { continue }
        let Ok(environment) = fs::read(process.path().join("environ")) else { continue };
        for entry in environment.split(|byte| *byte == 0) {
            for prefix in [b"STAMPEDE_SCRATCH=".as_slice(), b"STAMPEDE_CACHE=".as_slice()] {
                if let Some(value) = entry.strip_prefix(prefix).and_then(|value| std::str::from_utf8(value).ok()) {
                    paths.insert(PathBuf::from(value));
                }
            }
        }
    }
    paths
}

fn directories(path: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        entries => entries?,
    };
    entries.filter_map(|entry| match entry {
        Ok(entry) => match entry.file_type() {
            Ok(kind) if kind.is_dir() => Some(Ok(entry.path())),
            Ok(_) => None,
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => Some(Err(error)),
        },
        Err(error) => Some(Err(error)),
    }).collect()
}

fn size(path: &Path) -> io::Result<u64> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        metadata => metadata?,
    };
    if metadata.is_symlink() { return Ok(0) }
    let mut bytes = metadata.blocks().saturating_mul(512);
    if metadata.is_dir() {
        let entries = match fs::read_dir(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            entries => entries?,
        };
        for entry in entries { bytes = bytes.saturating_add(size(&entry?.path())?); }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scratch_survives_while_its_chat_record_exists_and_expires_afterward() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::new(root.path());
        fs::create_dir(root.path().join("chats")).unwrap();
        fs::write(root.path().join("chats/owner.json"), b"{}").unwrap();
        let session = storage.prepare("owner", root.path(), &BTreeMap::new()).await.unwrap();
        let scratch = session.scratch.clone();
        let cache = session.cache.clone();
        fs::write(scratch.join("keep"), b"work").unwrap();
        drop(session);
        storage.maintain(&config::Storage { cache_max_gib: 0, ..config::Storage::default() }).unwrap();
        assert!(scratch.join("keep").exists());
        assert!(cache.exists());
        fs::remove_file(root.path().join("chats/owner.json")).unwrap();
        storage.maintain(&config::Storage { cache_max_gib: 0, ..config::Storage::default() }).unwrap();
        assert!(!scratch.exists());
        assert!(!cache.exists());
    }

    #[tokio::test]
    async fn idle_caches_expire_by_age_and_size_but_leased_caches_survive() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::new(root.path());
        let session = storage.prepare("setup", root.path(), &BTreeMap::new()).await.unwrap();
        fs::write(session.cache.join("data"), vec![0; 8192]).unwrap();
        storage.trim(0, Duration::ZERO, &HashSet::new()).unwrap();
        assert!(session.cache.exists());
        let cache = session.cache.clone();
        drop(session);
        storage.trim(u64::MAX, Duration::ZERO, &HashSet::new()).unwrap();
        assert!(!cache.exists());
        let session = storage.prepare("setup-2", root.path(), &BTreeMap::new()).await.unwrap();
        let cache = session.cache.clone();
        drop(session);
        storage.trim(0, Duration::MAX, &HashSet::new()).unwrap();
        assert!(!cache.exists());
    }

    #[tokio::test]
    async fn maintenance_ignores_symlinks_and_keeps_a_live_setups_storage() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::new(root.path());
        let session = storage.prepare("setup", root.path(), &BTreeMap::new()).await.unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"source").unwrap();
        std::os::unix::fs::symlink(outside.path(), storage.root.join("cache/linked")).unwrap();
        storage.maintain(&config::Storage { cache_max_gib: 0, cache_max_age_days: 0, ..config::Storage::default() }).unwrap();
        assert!(session.scratch.exists());
        assert!(session.cache.exists());
        assert!(outside.path().join("keep").exists());
    }

    #[tokio::test]
    async fn rig_storage_overrides_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::new(root.path());
        let overrides = BTreeMap::from([("CARGO_TARGET_DIR".to_owned(), "/custom/target".to_owned())]);
        let session = storage.prepare("setup", root.path(), &overrides).await.unwrap();
        assert_eq!(session.env["CARGO_TARGET_DIR"], "/custom/target");
        assert_eq!(fs::canonicalize(&session.env["TMPDIR"]).unwrap(), session.scratch);
    }

    #[tokio::test]
    async fn tmpdir_is_a_short_alias_of_scratch_that_goes_with_it() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::new(root.path());
        let session = storage.prepare("setup", root.path(), &BTreeMap::new()).await.unwrap();
        let temp = PathBuf::from(&session.env["TMPDIR"]);
        assert!(temp.as_os_str().len() <= 40, "{}", temp.display());
        assert_eq!(fs::canonicalize(&temp).unwrap(), session.scratch);
        assert_eq!(session.env["CLAUDE_CODE_TMPDIR"], session.scratch.to_string_lossy());
        drop(session);
        assert!(fs::symlink_metadata(&temp).is_err());
    }

    #[test]
    fn low_space_reports_consumers_and_zero_disables_the_gate() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::new(root.path());
        fs::create_dir_all(root.path().join("storage/cache/test")).unwrap();
        fs::write(root.path().join("storage/cache/test/cache"), vec![0; 8192]).unwrap();
        let message = storage.pressure(root.path(), u64::MAX).unwrap().unwrap();
        assert!(message.contains("Largest consumers:"));
        assert!(message.contains("storage/cache"));
        assert_eq!(storage.pressure(root.path(), 0).unwrap(), None);
    }
}
