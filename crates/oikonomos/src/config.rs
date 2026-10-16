use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use tokio::sync::{mpsc, watch};
use ttf_parser::Tag;

use crate::themes::Theme;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    roots: Vec<PathBuf>,
    leader: String,
    scrollback: usize,
    #[serde(deserialize_with = "theme")]
    theme: Theme,
    font: Fonts,
    #[serde(deserialize_with = "rigs")]
    rig: Vec<Rig>,
    editor: Editor,
    keybar: Keybar,
    language_servers: LanguageServers,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: vec!["~/repos".into()],
            leader: "ctrl+space".to_owned(),
            scrollback: 10_000,
            theme: Theme::default(),
            font: Fonts::default(),
            rig: Vec::new(),
            editor: Editor::default(),
            keybar: Keybar::default(),
            language_servers: LanguageServers::default(),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Fonts {
    mono: Mono,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Mono {
    family: String,
    #[serde(deserialize_with = "faces")]
    files: Vec<Face>,
}

impl Default for Mono {
    fn default() -> Self {
        Self { family: "JetBrains Mono".to_owned(), files: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(default, deny_unknown_fields)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Keybar {
    pub extra: Vec<String>,
}

impl Default for Keybar {
    fn default() -> Self {
        Self { extra: vec!["|".to_owned(), "/".to_owned()] }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
    pub keybar: Keybar,
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
            keybar: Keybar::default(),
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
    fn parse(text: &str) -> Result<Self, toml::de::Error> {
        let Settings {
            roots,
            leader,
            scrollback,
            theme,
            font: Fonts { mono },
            rig: rigs,
            editor,
            keybar,
            language_servers,
        } = toml::from_str(text)?;
        Ok(Self {
            roots: roots.into_iter().map(expand).collect(),
            leader,
            scrollback,
            theme,
            font: Font { family: mono.family, faces: mono.files },
            rigs,
            editor,
            keybar,
            language_servers,
        })
    }

    fn load(path: &Path) -> Result<Self, Error> {
        let text = match fs::read_to_string(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            text => text?,
        };
        Ok(Self::parse(&text)?)
    }
}

#[derive(Serialize)]
pub struct Invalid {
    line: Option<usize>,
    message: String,
}

pub fn check(text: &str) -> Option<Invalid> {
    let error = Config::parse(text).err()?;
    let line = error.span().map(|span| text[..span.start].matches('\n').count() + 1);
    Some(Invalid { line, message: error.message().to_owned() })
}

pub fn write_theme(path: &Path, name: &str) -> Result<Theme, Error> {
    let theme = Theme::resolve(name)?;
    let text = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        text => text?,
    };
    let text = with_theme(text, name)?;
    fs::write(path, text)?;
    Ok(theme)
}

fn with_theme(mut text: String, name: &str) -> Result<String, Error> {
    #[derive(Deserialize)]
    struct Setting {
        theme: Option<toml::Spanned<String>>,
    }

    let setting: Setting = toml::from_str(&text)?;
    let value = toml::Value::String(name.to_owned()).to_string();
    match setting.theme {
        Some(theme) => text.replace_range(theme.span(), &value),
        None => text.insert_str(0, &format!("theme = {value}\n")),
    }
    Config::parse(&text)?;
    Ok(text)
}

fn theme<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Theme, D::Error> {
    Theme::resolve(&String::deserialize(deserializer)?).map_err(D::Error::custom)
}

fn rigs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Rig>, D::Error> {
    let rigs = Vec::<Rig>::deserialize(deserializer)?;
    let mut names = HashSet::new();
    match rigs.iter().find(|rig| !names.insert(&rig.name)) {
        Some(rig) => Err(D::Error::custom(format!("more than one rig is named {:?}", rig.name))),
        None => Ok(rigs),
    }
}

fn faces<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Face>, D::Error> {
    let files = Vec::<PathBuf>::deserialize(deserializer)?;
    files.into_iter().enumerate().map(face).collect::<Result<_, _>>().map_err(D::Error::custom)
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
