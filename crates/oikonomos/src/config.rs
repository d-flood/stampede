use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};
use ttf_parser::Tag;

use crate::themes::Theme;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Deserialize)]
#[serde(default)]
struct Settings {
    roots: Vec<PathBuf>,
    leader: String,
    scrollback: usize,
    theme: Option<String>,
    font: Fonts,
    rig: Vec<Rig>,
    editor: Editor,
    language_servers: LanguageServers,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: vec!["~/repos".into()],
            leader: "ctrl+space".to_owned(),
            scrollback: 10_000,
            theme: None,
            font: Fonts::default(),
            rig: Vec::new(),
            editor: Editor::default(),
            language_servers: LanguageServers::default(),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Fonts {
    mono: Mono,
}

#[derive(Deserialize)]
#[serde(default)]
struct Mono {
    family: String,
    files: Vec<PathBuf>,
}

impl Default for Mono {
    fn default() -> Self {
        Self { family: "JetBrains Mono".to_owned(), files: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rig {
    pub name: String,
    pub agent: Agent,
    #[serde(skip_serializing)]
    pub command: String,
    #[serde(default, skip_serializing)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing)]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing)]
    pub resume: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    ClaudeCode,
    Codex,
}

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude-code",
            Agent::Codex => "codex",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Editor {
    pub keymap: Keymap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Keymap {
    #[default]
    Standard,
    Vim,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LanguageServers {
    pub pins: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub roots: Vec<PathBuf>,
    pub leader: String,
    pub scrollback: usize,
    pub theme: Theme,
    pub font: Font,
    pub rigs: Vec<Rig>,
    pub editor: Editor,
    pub language_servers: LanguageServers,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            roots: Settings::default().roots.into_iter().map(expand).collect(),
            leader: Settings::default().leader,
            scrollback: Settings::default().scrollback,
            theme: Theme::default(),
            font: Font::default(),
            rigs: Vec::new(),
            editor: Editor::default(),
            language_servers: LanguageServers::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Font {
    pub family: String,
    pub faces: Vec<Face>,
}

impl Default for Font {
    fn default() -> Self {
        Self { family: Mono::default().family, faces: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Face {
    #[serde(skip)]
    pub path: PathBuf,
    pub url: String,
    pub weight: String,
    pub style: &'static str,
}

impl Config {
    fn load(path: &Path) -> Result<Self, Error> {
        let text = match fs::read_to_string(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            text => text?,
        };
        let Settings { roots, leader, scrollback, theme, font: Fonts { mono }, rig: rigs, editor, language_servers } =
            toml::from_str(&text)?;
        let faces = mono.files.into_iter().enumerate().map(face).collect::<Result<_, _>>()?;
        let mut names = HashSet::new();
        if let Some(rig) = rigs.iter().find(|rig| !names.insert(&rig.name)) {
            return Err(format!("more than one rig is named {:?}", rig.name).into());
        }
        Ok(Self {
            roots: roots.into_iter().map(expand).collect(),
            leader,
            scrollback,
            theme: Theme::resolve(theme.as_deref())?,
            font: Font { family: mono.family, faces },
            rigs,
            editor,
            language_servers,
        })
    }
}

pub fn expand(path: PathBuf) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => crate::home().join(rest),
        Err(_) => path,
    }
}

fn face((index, path): (usize, PathBuf)) -> Result<Face, Error> {
    let path = expand(path);
    let data = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let (weight, style) = match ttf_parser::Face::parse(&data, 0) {
        Ok(face) => {
            let wght = Tag::from_bytes(b"wght");
            let axis = face.variation_axes().into_iter().find(|axis| axis.tag == wght);
            let weight = match axis {
                Some(axis) => format!("{} {}", axis.min_value, axis.max_value),
                None => face.weight().to_number().to_string(),
            };
            (weight, if face.is_italic() { "italic" } else { "normal" })
        }
        Err(_) => ("400".to_owned(), "normal"),
    };
    Ok(Face { path, url: format!("/fonts/host/{index}"), weight, style })
}

pub fn watch(path: &Path) -> io::Result<watch::Receiver<Arc<Config>>> {
    let path = std::path::absolute(path)?;
    let config = Config::load(&path).unwrap_or_else(|error| {
        eprintln!("config {}: {error}", path.display());
        Config::default()
    });
    let (sender, receiver) = watch::channel(Arc::new(config));
    let dir = path.parent().unwrap_or(Path::new("/"));
    fs::create_dir_all(dir)?;
    let (events, mut pending) = mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = events.send(event);
    })
    .map_err(io::Error::other)?;
    watcher.watch(dir, RecursiveMode::NonRecursive).map_err(io::Error::other)?;
    tokio::spawn(async move {
        let _watcher = watcher;
        while let Some(event) = pending.recv().await {
            let Ok(notify::Event { kind, paths, .. }) = event else { continue };
            let ours = paths.iter().any(|changed| changed.file_name() == path.file_name());
            if kind.is_access() || !ours {
                continue;
            }
            // Editors save in several events; read once they settle.
            tokio::time::sleep(Duration::from_millis(50)).await;
            while pending.try_recv().is_ok() {}
            match Config::load(&path) {
                Ok(config) => {
                    sender.send_if_modified(|current| {
                        let changed = **current != config;
                        if changed {
                            *current = Arc::new(config);
                        }
                        changed
                    });
                }
                Err(error) => eprintln!("config {}: {error}", path.display()),
            }
        }
    });
    Ok(receiver)
}
