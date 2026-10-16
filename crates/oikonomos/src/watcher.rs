use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use ignore::{
    WalkBuilder,
    gitignore::{Gitignore, GitignoreBuilder},
};
use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, RemoveKind},
};
use serde::Serialize;
use tokio::sync::{broadcast, watch};

use crate::projects::{List, Project};

const DEBOUNCE: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "fs.changed", rename_all = "camelCase")]
pub struct Changed {
    pub project_id: String,
    pub paths: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub git: bool,
}

enum Signal {
    Event(notify::Result<Event>),
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

fn start(project: Project, changes: broadcast::Sender<Changed>) -> notify::Result<mpsc::Sender<Signal>> {
    let (signals, pending) = mpsc::channel();
    let events = signals.clone();
    let watcher = notify::recommended_watcher(move |event| {
        let _ = events.send(Signal::Event(event));
    })?;
    thread::spawn(move || {
        let mut tree = Tree::new(PathBuf::from(&project.path), watcher);
        while let Ok(signal) = pending.recv() {
            let first = match signal {
                Signal::Event(event) => event,
                Signal::Open(path) => { tree.open(path); continue; }
                Signal::Stop => return,
            };
            let mut batch = vec![first];
            let deadline = Instant::now() + DEBOUNCE;
            loop {
                match pending.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(Signal::Event(event)) => batch.push(event),
                    Ok(Signal::Open(path)) => tree.open(path),
                    Err(RecvTimeoutError::Timeout) => break,
                    Ok(Signal::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            let (paths, git) = tree.absorb(batch);
            if !paths.is_empty() || git {
                let paths = paths.into_iter().collect();
                let _ = changes.send(Changed { project_id: project.id.clone(), paths, git });
            }
        }
    });
    Ok(signals)
}

struct Tree {
    root: PathBuf,
    git_dir: Option<PathBuf>,
    watcher: RecommendedWatcher,
    watched: HashSet<PathBuf>,
    opened: HashSet<PathBuf>,
    ignores: HashMap<PathBuf, Gitignore>,
    excludes: [Gitignore; 2],
}

impl Tree {
    fn new(root: PathBuf, mut watcher: RecommendedWatcher) -> Self {
        let git_dir = git_dir(&root);
        if let Some(dir) = &git_dir
            && let Err(error) = watcher.watch(dir, RecursiveMode::NonRecursive)
        {
            eprintln!("cannot watch {}: {error}", dir.display());
        }
        let mut exclude = GitignoreBuilder::new(&root);
        exclude.add(root.join(".git/info/exclude"));
        let exclude = exclude.build().unwrap_or_else(|_| Gitignore::empty());
        let global = GitignoreBuilder::new(&root).build_global().0;
        let mut tree = Self {
            root,
            git_dir,
            watcher,
            watched: HashSet::new(),
            opened: HashSet::new(),
            ignores: HashMap::new(),
            excludes: [exclude, global],
        };
        tree.scan(&tree.root.clone());
        tree
    }

    fn absorb(&mut self, batch: Vec<notify::Result<Event>>) -> (BTreeSet<String>, bool) {
        let mut paths = BTreeSet::new();
        let mut git = false;
        let mut rescan = false;
        for Event { kind, paths: changed, .. } in batch.into_iter().flatten() {
            if kind.is_access() {
                continue;
            }
            for path in changed {
                if self.git_dir.as_deref().is_some_and(|dir| path.parent() == Some(dir)) {
                    git |= path.file_name().is_some_and(|name| name == "HEAD" || name == "index");
                    continue;
                }
                let Some(rel) = self.relative(&path) else { continue };
                if rel.is_empty() || Path::new(&rel).starts_with(".git") {
                    continue;
                }
                let dir = match kind {
                    EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder) => true,
                    _ => path.is_dir(),
                };
                if self.ignored(&path, dir) && !self.opened.iter().any(|opened| opened.starts_with(&path)) {
                    continue;
                }
                rescan |= path.file_name().is_some_and(|name| name == ".gitignore");
                if self.watched.contains(&path) {
                    if !path.exists() {
                        self.forget(&path);
                    }
                } else if dir {
                    paths.extend(self.scan(&path));
                }
                paths.insert(rel);
            }
        }
        if rescan {
            for dir in self.watched.drain() {
                let _ = self.watcher.unwatch(&dir);
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
            if !self.watched.contains(parent) {
                match self.watcher.watch(parent, RecursiveMode::NonRecursive) {
                    Ok(()) => { self.watched.insert(parent.to_owned()); }
                    Err(error) => eprintln!("cannot watch {}: {error}", parent.display()),
                }
            }
        }
    }

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
            match self.watcher.watch(path, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    self.watched.insert(path.to_owned());
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
        self.watched.retain(|dir| {
            let gone = dir.starts_with(path);
            if gone {
                let _ = self.watcher.unwatch(dir);
            }
            !gone
        });
        self.ignores.retain(|dir, _| !dir.starts_with(path));
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
