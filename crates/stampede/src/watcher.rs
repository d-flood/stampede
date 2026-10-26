use std::{
    collections::{BTreeSet, HashMap, HashSet},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use ignore::{
    WalkBuilder,
    gitignore::{Gitignore, GitignoreBuilder},
};
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask, Watches};
use serde::Serialize;
use tokio::sync::{broadcast, watch};

use crate::projects::{List, Project};

const DEBOUNCE: Duration = Duration::from_millis(100);
const CHANGES: WatchMask = WatchMask::ATTRIB
    .union(WatchMask::CREATE)
    .union(WatchMask::DELETE)
    .union(WatchMask::MODIFY)
    .union(WatchMask::MOVED_FROM)
    .union(WatchMask::MOVED_TO)
    .union(WatchMask::DELETE_SELF)
    .union(WatchMask::MOVE_SELF);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "fs.changed", rename_all = "camelCase")]
pub struct Changed {
    pub project_id: String,
    pub paths: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub git: bool,
}

struct Event {
    wd: WatchDescriptor,
    mask: EventMask,
    name: Option<OsString>,
}

enum Signal {
    Events(Vec<Event>),
    Open(PathBuf),
    Stop,
}

pub type Opened = tokio::sync::mpsc::UnboundedSender<(String, PathBuf)>;

pub fn spawn(mut projects: watch::Receiver<List>) -> (broadcast::Sender<Changed>, Opened) {
    let (changes, _) = broadcast::channel(256);
    let sender = changes.clone();
    let (opened, mut openings) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut watching = HashMap::<String, mpsc::Sender<Signal>>::new();
        loop {
            let list = projects.borrow_and_update().clone();
            watching.retain(|id, stop| {
                let open = list.iter().any(|project| project.id == *id);
                if !open {
                    let _ = stop.send(Signal::Stop);
                }
                open
            });
            for project in list.iter() {
                if watching.contains_key(&project.id) {
                    continue;
                }
                match start(project.clone(), sender.clone()) {
                    Ok(stop) => {
                        watching.insert(project.id.clone(), stop);
                    }
                    Err(error) => eprintln!("cannot watch {}: {error}", project.path),
                }
            }
            tokio::select! {
                changed = projects.changed() => if changed.is_err() { break; },
                Some((project, path)) = openings.recv() => {
                    if let Some(watcher) = watching.get(&project) {
                        let _ = watcher.send(Signal::Open(path));
                    }
                }
            }
        }
    });
    (changes, opened)
}

fn start(project: Project, changes: broadcast::Sender<Changed>) -> io::Result<mpsc::Sender<Signal>> {
    let (signals, pending) = mpsc::channel();
    let mut inotify = Inotify::init()?;
    let watches = inotify.watches();
    let events = signals.clone();
    thread::spawn(move || {
        let mut buffer = [0; 4096];
        loop {
            let read = match inotify.read_events_blocking(&mut buffer) {
                Ok(read) => read.map(|event| Event { wd: event.wd, mask: event.mask, name: event.name.map(OsString::from) }).collect(),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            if events.send(Signal::Events(read)).is_err() {
                break;
            }
        }
    });
    thread::spawn(move || {
        let mut tree = Tree::new(PathBuf::from(&project.path), watches);
        'signals: while let Ok(signal) = pending.recv() {
            let mut batch = match signal {
                Signal::Events(events) => events,
                Signal::Open(path) => { tree.open(path); continue; }
                Signal::Stop => break,
            };
            let deadline = Instant::now() + DEBOUNCE;
            loop {
                match pending.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(Signal::Events(events)) => batch.extend(events),
                    Ok(Signal::Open(path)) => tree.open(path),
                    Err(RecvTimeoutError::Timeout) => break,
                    Ok(Signal::Stop) | Err(RecvTimeoutError::Disconnected) => break 'signals,
                }
            }
            let (paths, git) = tree.absorb(batch);
            if !paths.is_empty() || git {
                let paths = paths.into_iter().collect();
                let _ = changes.send(Changed { project_id: project.id.clone(), paths, git });
            }
        }
        // Removing the watches wakes the reader, which then stops on the closed channel.
        drop(pending);
        tree.close();
    });
    Ok(signals)
}

struct Tree {
    root: PathBuf,
    git: Option<WatchDescriptor>,
    watches: Watches,
    dirs: HashMap<WatchDescriptor, PathBuf>,
    opened: HashSet<PathBuf>,
    ignores: HashMap<PathBuf, Gitignore>,
    excludes: [Gitignore; 2],
}

impl Tree {
    fn new(root: PathBuf, mut watches: Watches) -> Self {
        let git = git_dir(&root).and_then(|dir| {
            watches.add(&dir, CHANGES).inspect_err(|error| eprintln!("cannot watch {}: {error}", dir.display())).ok()
        });
        let mut exclude = GitignoreBuilder::new(&root);
        exclude.add(root.join(".git/info/exclude"));
        let exclude = exclude.build().unwrap_or_else(|_| Gitignore::empty());
        let global = GitignoreBuilder::new(&root).build_global().0;
        let mut tree = Self {
            root,
            git,
            watches,
            dirs: HashMap::new(),
            opened: HashSet::new(),
            ignores: HashMap::new(),
            excludes: [exclude, global],
        };
        tree.scan(&tree.root.clone());
        tree
    }

    fn absorb(&mut self, batch: Vec<Event>) -> (BTreeSet<String>, bool) {
        let mut paths = BTreeSet::new();
        let mut git = false;
        let mut rescan = false;
        for Event { wd, mask, name } in batch {
            if mask.contains(EventMask::IGNORED) {
                self.dirs.remove(&wd);
                continue;
            }
            if self.git.as_ref() == Some(&wd) {
                git |= name.is_some_and(|name| name == "HEAD" || name == "index" || name == "config");
                continue;
            }
            let Some(parent) = self.dirs.get(&wd) else { continue };
            let path = match name {
                Some(name) => parent.join(name),
                None => parent.clone(),
            };
            let Some(rel) = self.relative(&path) else { continue };
            if rel.is_empty() || Path::new(&rel).starts_with(".git") {
                continue;
            }
            let dir = mask.contains(EventMask::ISDIR) || path.is_dir();
            if self.ignored(&path, dir) && !self.opened.iter().any(|opened| opened.starts_with(&path)) {
                continue;
            }
            rescan |= path.file_name().is_some_and(|name| name == ".gitignore");
            let gone = mask.intersects(EventMask::DELETE_SELF | EventMask::MOVE_SELF)
                || mask.contains(EventMask::ISDIR) && mask.intersects(EventMask::DELETE | EventMask::MOVED_FROM);
            if gone {
                self.forget(&path);
            } else if dir {
                paths.extend(self.scan(&path));
            }
            paths.insert(rel);
        }
        if rescan {
            for (wd, _) in self.dirs.drain() {
                let _ = self.watches.remove(wd);
            }
            self.ignores.clear();
            self.scan(&self.root.clone());
            for path in self.opened.clone() { self.open(path); }
        }
        (paths, git)
    }

    fn open(&mut self, path: PathBuf) {
        self.opened.insert(path.clone());
        for parent in path.ancestors().skip(1).take_while(|parent| parent.starts_with(&self.root)) {
            match self.watches.add(parent, CHANGES) {
                Ok(wd) => { self.dirs.insert(wd, parent.to_owned()); }
                Err(error) => eprintln!("cannot watch {}: {error}", parent.display()),
            }
        }
    }

    /// Watches `dir` and what it holds, giving its files; nothing when `dir` was already watched.
    fn scan(&mut self, dir: &Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut failure = None;
        let walk = WalkBuilder::new(dir)
            .hidden(false)
            .require_git(false)
            .filter_entry(|entry| entry.file_name() != ".git")
            .build();
        for entry in walk.flatten() {
            let path = entry.path();
            if !entry.file_type().is_some_and(|kind| kind.is_dir()) {
                files.extend(self.relative(path));
                continue;
            }
            match self.watches.add(path, CHANGES) {
                Ok(wd) => {
                    if self.dirs.insert(wd, path.to_owned()).is_some() && path == dir {
                        return files;
                    }
                }
                Err(error) => failure = Some(error),
            }
            let gitignore = path.join(".gitignore");
            if gitignore.is_file() {
                self.ignores.insert(path.to_owned(), Gitignore::new(gitignore).0);
            }
        }
        if let Some(error) = failure {
            eprintln!("cannot watch all of {}: {error}", dir.display());
        }
        files
    }

    fn forget(&mut self, path: &Path) {
        self.dirs.retain(|wd, dir| {
            let gone = dir.starts_with(path);
            if gone {
                let _ = self.watches.remove(wd.clone());
            }
            !gone
        });
        self.ignores.retain(|dir, _| !dir.starts_with(path));
    }

    fn close(mut self) {
        for wd in self.dirs.into_keys().chain(self.git) {
            let _ = self.watches.remove(wd);
        }
    }

    fn ignored(&self, path: &Path, dir: bool) -> bool {
        let local = path
            .ancestors()
            .skip(1)
            .take_while(|ancestor| ancestor.starts_with(&self.root))
            .filter_map(|ancestor| self.ignores.get(ancestor));
        for matcher in local.chain(&self.excludes) {
            let matched = matcher.matched(path, dir);
            if !matched.is_none() {
                return matched.is_ignore();
            }
        }
        false
    }

    fn relative(&self, path: &Path) -> Option<String> {
        Some(path.strip_prefix(&self.root).ok()?.to_str()?.to_owned())
    }
}

fn git_dir(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let link = fs::read_to_string(&dot_git).ok()?;
    let dir = root.join(link.strip_prefix("gitdir:")?.trim());
    dir.is_dir().then_some(dir)
}
