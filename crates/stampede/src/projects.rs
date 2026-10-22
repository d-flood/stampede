use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

use crate::config::expand;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<Worktree>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Worktree {
    pub parent: String,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Kind {
    Epic { epic: String, role: Role },
    Branch {
        branch: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        existing: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Integration,
    Ticket(u32),
}

pub type List = Arc<Vec<Project>>;

pub struct Projects {
    file: PathBuf,
    list: watch::Sender<List>,
}

impl Projects {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        let file = data_dir.join("projects.json");
        let mut list: Vec<Project> = match fs::read(&file) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            bytes => serde_json::from_slice(&bytes?)?,
        };
        for project in &mut list {
            project.icon = icon(data_dir, &project.id);
        }
        Ok(Self { file, list: watch::Sender::new(Arc::new(list)) })
    }

    pub fn list(&self) -> List {
        self.list.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<List> {
        self.list.subscribe()
    }

    pub fn open(&self, path: &str) -> Result<String, String> {
        let path = expand(PathBuf::from(path));
        if !path.is_absolute() {
            return Err("Not an absolute path".to_owned());
        }
        let path = match fs::canonicalize(&path) {
            Ok(path) if path.is_dir() => path,
            Ok(_) => return Err("Not a directory".to_owned()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err("No such directory".to_owned());
            }
            Err(error) => return Err(error.to_string()),
        };
        let path = path.into_os_string().into_string().map_err(|_| "Path is not UTF-8")?;
        let id = id(&path);
        self.change(|list| {
            if list.iter().any(|project| project.id == id) {
                return false;
            }
            list.push(Project { icon: icon(self.file.parent().unwrap(), &id), id: id.clone(), path, worktree: None });
            true
        });
        Ok(id)
    }

    pub fn nest(&self, path: &str, worktree: Worktree) -> Project {
        let id = id(path);
        let mut nested = Project { id: id.clone(), path: path.to_owned(), icon: None, worktree: Some(worktree) };
        self.change(|list| match list.iter().find(|project| project.id == id) {
            Some(open) => {
                nested = open.clone();
                false
            }
            None => {
                list.push(nested.clone());
                true
            }
        });
        nested
    }

    pub fn close(&self, id: &str) {
        self.change(|list| {
            let before = list.len();
            list.retain(|project| project.id != id);
            list.len() != before
        });
    }

    pub fn icon_dir(&self) -> PathBuf {
        self.file.parent().unwrap().join("project-icons")
    }

    pub fn set_icon(&self, id: &str, bytes: Option<&[u8]>) -> io::Result<()> {
        let mut result = Ok(());
        self.change(|list| {
            let Some(project) = list.iter_mut().find(|project| project.id == id) else {
                result = Err(io::Error::new(io::ErrorKind::NotFound, "Project is not open"));
                return false;
            };
            let save = || -> io::Result<Option<String>> {
                let directory = self.icon_dir();
                fs::create_dir_all(&directory)?;
                let asset = match bytes {
                    Some(bytes) => {
                        let hash: String = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
                        let asset = format!("{hash}.png");
                        let temporary = directory.join(format!("{id}.png.tmp"));
                        fs::write(&temporary, bytes)?;
                        fs::rename(temporary, directory.join(&asset))?;
                        Some(asset)
                    }
                    None => None,
                };
                let temporary = directory.join(format!("{id}.json.tmp"));
                fs::write(&temporary, serde_json::to_vec(&asset)?)?;
                fs::rename(temporary, directory.join(format!("{id}.json")))?;
                Ok(asset)
            };
            match save() {
                Ok(icon) => { project.icon = icon; true }
                Err(error) => { result = Err(error); false }
            }
        });
        result
    }

    pub fn reorder(&self, order: &[String]) {
        self.change(|list| {
            let before = list.clone();
            list.sort_by_key(|project| {
                order.iter().position(|id| *id == project.id).unwrap_or(usize::MAX)
            });
            *list != before
        });
    }

    fn change(&self, modify: impl FnOnce(&mut Vec<Project>) -> bool) {
        self.list.send_if_modified(|list| {
            let changed = modify(Arc::make_mut(list));
            if changed && let Err(error) = save(&self.file, list) {
                eprintln!("cannot save {}: {error}", self.file.display());
            }
            changed
        });
    }
}

fn icon(data_dir: &Path, id: &str) -> Option<String> {
    serde_json::from_slice(&fs::read(data_dir.join("project-icons").join(format!("{id}.json"))).ok()?).ok()?
}

fn save(file: &Path, list: &[Project]) -> io::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(list)?)?;
    fs::rename(tmp, file)
}

fn id(path: &str) -> String {
    Sha256::digest(path)[..6].iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn directories(roots: &[PathBuf]) -> Vec<String> {
    let home = crate::home();
    let mut directories = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        let mut children: Vec<_> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.is_dir())
            .filter_map(|path| {
                let path = match path.strip_prefix(&home) {
                    Ok(rest) => Path::new("~").join(rest),
                    Err(_) => path,
                };
                path.into_os_string().into_string().ok()
            })
            .collect();
        children.sort();
        directories.extend(children);
    }
    directories
}
