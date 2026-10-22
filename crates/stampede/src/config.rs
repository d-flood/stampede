use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
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
    name: Option<String>,
    roots: Vec<PathBuf>,
    leader: String,
    scrollback: usize,
    #[serde(deserialize_with = "theme")]
    theme: Theme,
    font: Fonts,
    ui_font: Option<String>,
    #[serde(deserialize_with = "rigs")]
    rig: Vec<Rig>,
    commit_rig: Option<String>,
    commit_instructions: Option<String>,
    #[serde(deserialize_with = "modes")]
    modes: BTreeMap<Agent, Vec<String>>,
    #[serde(deserialize_with = "clips")]
    clip: Vec<Clip>,
    editor: Editor,
    keybar: Keybar,
    language_servers: LanguageServers,
    r#loop: Loop,
    storage: Storage,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            name: None,
            roots: vec!["~/repos".into()],
            leader: "ctrl+;".to_owned(),
            scrollback: 10_000,
            theme: Theme::default(),
            font: Fonts::default(),
            ui_font: None,
            rig: Vec::new(),
            commit_rig: None,
            commit_instructions: None,
            modes: BTreeMap::new(),
            clip: Vec::new(),
            editor: Editor::default(),
            keybar: Keybar::default(),
            language_servers: LanguageServers::default(),
            r#loop: Loop::default(),
            storage: Storage::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Loop {
    #[serde(deserialize_with = "fraction")]
    pause_at: f64,
}

impl Default for Loop {
    fn default() -> Self {
        Self { pause_at: 0.9 }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Storage {
    pub min_free_gib: u64,
    pub cache_max_gib: u64,
    pub cache_max_age_days: u64,
}

impl Default for Storage {
    fn default() -> Self {
        Self { min_free_gib: 2, cache_max_gib: 8, cache_max_age_days: 7 }
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
    #[serde(default, skip_serializing)]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl Rig {
    fn starter(agent: Agent) -> Self {
        let (name, mode) = match agent {
            Agent::ClaudeCode => ("Claude Code", "auto"),
            Agent::Codex => ("Codex", "approve-for-me"),
            Agent::Copilot => ("GitHub Copilot", "default"),
        };
        Self {
            name: name.to_owned(),
            agent,
            env: BTreeMap::new(),
            shortcut: None,
            model: None,
            effort: None,
            mode: Some(mode.to_owned()),
        }
    }

    pub fn settings(&self) -> AgentSettings {
        let mode = self.mode.clone().or_else(|| Some(match self.agent {
            Agent::ClaudeCode => "auto",
            Agent::Codex => "approve-for-me",
            Agent::Copilot => "default",
        }.to_owned()));
        AgentSettings { model: self.model.clone(), effort: self.effort.clone(), mode }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub name: String,
    pub text: String,
}

impl Clip {
    fn check(&self) -> Result<(), &'static str> {
        if self.name.is_empty() {
            Err("Clip name is required")
        } else if self.name.contains(char::is_whitespace) {
            Err("Clip names cannot contain spaces")
        } else if self.text.trim().is_empty() {
            Err("Clip text is required")
        } else {
            Ok(())
        }
    }
}

/// The model, effort and mode an Agent starts with.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl AgentSettings {
    pub fn claude_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        for (flag, value) in [("--model", &self.model), ("--effort", &self.effort), ("--permission-mode", &self.mode)] {
            if let Some(value) = value {
                flags.extend([flag.to_owned(), value.clone()]);
            }
        }
        flags
    }
}

pub fn codex_policy(mode: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match mode {
        "read-only" => Some(("on-request", "read-only", "user")),
        "default" => Some(("on-request", "workspace-write", "user")),
        "approve-for-me" => Some(("on-request", "workspace-write", "auto_review")),
        "full-access" => Some(("never", "danger-full-access", "user")),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    ClaudeCode,
    Codex,
    Copilot,
}

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude-code",
            Agent::Codex => "codex",
            Agent::Copilot => "copilot",
        }
    }

    /// The modes a Rig can start this Agent in, by the Agent's own names.
    pub fn modes(self) -> &'static [&'static str] {
        match self {
            Agent::ClaudeCode => &["default", "acceptEdits", "plan", "auto", "dontAsk", "bypassPermissions"],
            Agent::Codex => &["read-only", "default", "approve-for-me", "full-access"],
            Agent::Copilot => &["default", "full-access"],
        }
    }

    pub fn chat_modes(self) -> Vec<&'static str> {
        let mut modes = self.modes().to_vec();
        if self == Agent::Codex {
            modes.push("plan");
        }
        modes
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
    pub name: Option<String>,
    pub roots: Vec<PathBuf>,
    pub leader: String,
    pub scrollback: usize,
    pub theme: Theme,
    pub font: Font,
    pub ui_font: Option<String>,
    pub rigs: Vec<Rig>,
    pub commit_rig: Option<String>,
    pub commit_instructions: Option<String>,
    pub modes: BTreeMap<Agent, Vec<String>>,
    pub clips: Vec<Clip>,
    pub editor: Editor,
    pub keybar: Keybar,
    pub language_servers: LanguageServers,
    pub pause_at: f64,
    pub storage: Storage,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: None,
            roots: Settings::default().roots.into_iter().map(expand).collect(),
            leader: Settings::default().leader,
            scrollback: Settings::default().scrollback,
            theme: Theme::default(),
            font: Font::default(),
            ui_font: None,
            rigs: Vec::new(),
            commit_rig: None,
            commit_instructions: None,
            modes: BTreeMap::new(),
            clips: Vec::new(),
            editor: Editor::default(),
            keybar: Keybar::default(),
            language_servers: LanguageServers::default(),
            pause_at: Loop::default().pause_at,
            storage: Storage::default(),
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
            name,
            roots,
            leader,
            scrollback,
            theme,
            font: Fonts { mono },
            ui_font,
            rig: rigs,
            commit_rig,
            commit_instructions,
            modes,
            clip: clips,
            editor,
            keybar,
            language_servers,
            r#loop,
            storage,
        } = toml::from_str(text)?;
        Ok(Self {
            name: name.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()),
            roots: roots.into_iter().map(expand).collect(),
            leader,
            scrollback,
            theme,
            font: Font { family: mono.family, faces: mono.files },
            ui_font: ui_font.map(|family| family.trim().to_owned()).filter(|family| !family.is_empty()),
            rigs,
            commit_rig,
            commit_instructions: commit_instructions.map(|text| text.trim().to_owned()).filter(|text| !text.is_empty()),
            modes,
            clips,
            editor,
            keybar,
            language_servers,
            pause_at: r#loop.pause_at,
            storage,
        })
    }

    /// The Rig that drafts commit messages: the chosen one, else the first.
    pub fn commit_rig(&self) -> Option<&Rig> {
        let chosen = self.commit_rig.as_ref().and_then(|name| self.rigs.iter().find(|rig| rig.name == *name));
        chosen.or(self.rigs.first())
    }

    pub fn commit_instructions(&self) -> &str {
        self.commit_instructions.as_deref().unwrap_or(crate::templates::COMMIT_INSTRUCTIONS)
    }

    pub fn available_modes(&self, agent: Agent) -> Vec<&'static str> {
        let chosen = self.modes.get(&agent);
        agent.chat_modes().into_iter().filter(|mode| chosen.is_none_or(|chosen| chosen.iter().any(|value| value == mode))).collect()
    }

    /// The first Rig for an Agent, or the Agent's defaults when no Rig runs it.
    pub fn rig_for(&self, agent: Agent) -> Rig {
        self.rigs.iter().find(|rig| rig.agent == agent).cloned().unwrap_or_else(|| Rig::starter(agent))
    }

    fn seed_rigs(path: &Path) -> Result<(), Error> {
        let mut text = match fs::read_to_string(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            text => text?,
        };
        Self::parse(&text)?;
        let document: toml::Table = toml::from_str(&text)?;
        if document.contains_key("rig") {
            return Ok(());
        }
        let rigs = [Agent::ClaudeCode, Agent::Codex].map(Rig::starter);
        let addition = toml::to_string(&toml::Table::from_iter([
            ("rig".to_owned(), toml::Value::try_from(rigs)?),
        ]))?;
        text.push_str(&format!("\n{addition}"));
        fs::create_dir_all(path.parent().unwrap_or(Path::new("/")))?;
        fs::write(path, text)?;
        Ok(())
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

pub fn write_host_name(path: &Path, name: Option<&str>) -> Result<Option<String>, Error> {
    let name = name.map(str::trim).filter(|name| !name.is_empty());
    let text = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        text => text?,
    };
    let text = with_host_name(text, name)?;
    fs::write(path, text)?;
    Ok(name.map(str::to_owned))
}

pub fn write_ui_font(path: &Path, family: Option<&str>) -> Result<(), Error> {
    let text = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        text => text?,
    };
    fs::write(path, with_top_level(text, "ui_font", family)?)?;
    Ok(())
}

pub fn write_commit_rig(path: &Path, name: &str) -> Result<String, Error> {
    let text = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        text => text?,
    };
    if !Config::parse(&text)?.rigs.iter().any(|rig| rig.name == name) {
        return Err(format!("there is no Rig {name}").into());
    }
    fs::write(path, with_top_level(text, "commit_rig", Some(name))?)?;
    Ok(name.to_owned())
}

pub fn write_commit_instructions(path: &Path, text: &str) -> Result<String, Error> {
    let chosen = Some(text.trim()).filter(|text| !text.is_empty() && *text != crate::templates::COMMIT_INSTRUCTIONS);
    let current = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        current => current?,
    };
    fs::write(path, with_top_level(current, "commit_instructions", chosen)?)?;
    Ok(chosen.unwrap_or(crate::templates::COMMIT_INSTRUCTIONS).to_owned())
}

pub fn write_modes(path: &Path, agent: Agent, modes: &[String]) -> Result<Config, Error> {
    let text = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        text => text?,
    };
    let text = with_modes(text, agent, modes)?;
    let updated = Config::parse(&text)?;
    fs::write(path, text)?;
    Ok(updated)
}

pub fn write_rig(path: &Path, original: Option<&str>, mut rig: Rig) -> Result<Vec<Rig>, Error> {
    let mut text = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        text => text?,
    };
    let current = Config::parse(&text)?;
    rig.name = rig.name.trim().to_owned();
    if rig.name.is_empty() {
        return Err("Rig name is required".into());
    }
    if current.rigs.iter().any(|other| other.name == rig.name && Some(other.name.as_str()) != original) {
        return Err("A Rig with this name already exists".into());
    }
    if let Some(name) = original {
        let old = current.rigs.iter().find(|rig| rig.name == name).ok_or("Rig no longer exists; reopen the editor")?;
        rig.env = if old.agent == rig.agent { old.env.clone() } else { current.rig_for(rig.agent).env };
        rig.shortcut = old.shortcut.clone();
    } else {
        rig.env = current.rig_for(rig.agent).env;
    }
    let mut value = toml::Value::try_from(&rig)?;
    if !rig.env.is_empty() {
        value.as_table_mut().unwrap().insert("env".to_owned(), toml::Value::try_from(&rig.env)?);
    }
    let replacement = toml::to_string(&toml::Table::from_iter([
        ("rig".to_owned(), toml::Value::Array(vec![value])),
    ]))?;
    if let Some(name) = original {
        let index = current.rigs.iter().position(|rig| rig.name == name).unwrap();
        text.replace_range(rig_range(&text, index)?, replacement.trim_end());
    } else {
        if current.rigs.is_empty() {
            let document: BTreeMap<toml::Spanned<String>, toml::Spanned<toml::Value>> = toml::from_str(&text)?;
            if let Some((key, value)) = document.iter().find(|(key, _)| key.get_ref() == "rig") {
                text.replace_range(key.span().start..value.span().end, "");
            }
        }
        text.push_str(&format!("\n{replacement}"));
    }
    if original.is_some_and(|name| current.commit_rig.as_deref() == Some(name) && name != rig.name) {
        text = with_top_level(text, "commit_rig", Some(&rig.name))?;
    }
    let updated = Config::parse(&text)?;
    fs::write(path, text)?;
    Ok(updated.rigs)
}

pub fn reorder_rigs(path: &Path, order: &[String]) -> Result<Vec<Rig>, Error> {
    let mut text = fs::read_to_string(path)?;
    let current = Config::parse(&text)?;
    let names: HashSet<_> = current.rigs.iter().map(|rig| &rig.name).collect();
    if order.len() != names.len() || order.iter().collect::<HashSet<_>>() != names {
        return Err("Rig list changed; reopen Settings and try again".into());
    }
    let ranges = (0..current.rigs.len()).map(|index| rig_range(&text, index)).collect::<Result<Vec<_>, _>>()?;
    let replacements = order.iter().map(|name| {
        let index = current.rigs.iter().position(|rig| rig.name == *name).unwrap();
        text[ranges[index].clone()].to_owned()
    }).collect::<Vec<_>>();
    for (range, replacement) in ranges.into_iter().zip(replacements).rev() {
        text.replace_range(range, &replacement);
    }
    let updated = Config::parse(&text)?;
    fs::write(path, text)?;
    Ok(updated.rigs)
}

pub fn delete_rig(path: &Path, name: &str) -> Result<Vec<Rig>, Error> {
    let mut text = fs::read_to_string(path)?;
    let current = Config::parse(&text)?;
    let index = current.rigs.iter().position(|rig| rig.name == name).ok_or("Rig no longer exists; reopen the editor")?;
    text.replace_range(rig_range(&text, index)?, "");
    if current.rigs.len() == 1 {
        text.insert_str(0, "rig = []\n");
    }
    if current.commit_rig.as_deref() == Some(name) {
        text = with_top_level(text, "commit_rig", None)?;
    }
    let updated = Config::parse(&text)?;
    fs::write(path, text)?;
    Ok(updated.rigs)
}

fn rig_range(text: &str, index: usize) -> Result<std::ops::Range<usize>, Error> {
    #[derive(Deserialize)]
    struct Document { rig: Vec<toml::Spanned<BTreeMap<String, toml::Spanned<toml::Value>>>> }
    #[derive(Deserialize)]
    struct Environments { rig: Vec<Environment> }
    #[derive(Deserialize)]
    struct Environment { #[serde(default)] env: BTreeMap<String, toml::Spanned<String>> }
    let document: Document = toml::from_str(text)?;
    let environments: Environments = toml::from_str(text)?;
    let old = &document.rig[index];
    let end = old.get_ref().values().map(|field| field.span().end)
        .chain(environments.rig[index].env.values().map(|field| field.span().end))
        .max().unwrap_or(old.span().end);
    Ok(old.span().start..end)
}

pub fn write_clip(path: &Path, original: Option<&str>, mut clip: Clip) -> Result<Vec<Clip>, Error> {
    let mut text = match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        text => text?,
    };
    let current = Config::parse(&text)?;
    clip.name = clip.name.trim().to_owned();
    clip.check()?;
    if current.clips.iter().any(|other| other.name == clip.name && Some(other.name.as_str()) != original) {
        return Err("A Clip with this name already exists".into());
    }
    let replacement = toml::to_string(&toml::Table::from_iter([
        ("clip".to_owned(), toml::Value::try_from([&clip])?),
    ]))?;
    match original {
        Some(name) => {
            let index = current.clips.iter().position(|clip| clip.name == name).ok_or("Clip no longer exists; reopen the editor")?;
            text.replace_range(clip_range(&text, index)?, replacement.trim_end());
        }
        None => text.push_str(&format!("\n{replacement}")),
    }
    let updated = Config::parse(&text)?;
    fs::write(path, text)?;
    Ok(updated.clips)
}

pub fn reorder_clips(path: &Path, order: &[String]) -> Result<Vec<Clip>, Error> {
    let mut text = fs::read_to_string(path)?;
    let current = Config::parse(&text)?;
    let names: HashSet<_> = current.clips.iter().map(|clip| &clip.name).collect();
    if order.len() != names.len() || order.iter().collect::<HashSet<_>>() != names {
        return Err("Clip list changed; reopen Settings and try again".into());
    }
    let ranges = (0..current.clips.len()).map(|index| clip_range(&text, index)).collect::<Result<Vec<_>, _>>()?;
    let replacements = order.iter().map(|name| {
        let index = current.clips.iter().position(|clip| clip.name == *name).unwrap();
        text[ranges[index].clone()].to_owned()
    }).collect::<Vec<_>>();
    for (range, replacement) in ranges.into_iter().zip(replacements).rev() {
        text.replace_range(range, &replacement);
    }
    let updated = Config::parse(&text)?;
    fs::write(path, text)?;
    Ok(updated.clips)
}

pub fn delete_clip(path: &Path, name: &str) -> Result<Vec<Clip>, Error> {
    let mut text = fs::read_to_string(path)?;
    let current = Config::parse(&text)?;
    let index = current.clips.iter().position(|clip| clip.name == name).ok_or("Clip no longer exists; reopen the editor")?;
    text.replace_range(clip_range(&text, index)?, "");
    let updated = Config::parse(&text)?;
    fs::write(path, text)?;
    Ok(updated.clips)
}

fn clip_range(text: &str, index: usize) -> Result<std::ops::Range<usize>, Error> {
    #[derive(Deserialize)]
    struct Document { clip: Vec<toml::Spanned<BTreeMap<String, toml::Spanned<toml::Value>>>> }
    let document: Document = toml::from_str(text)?;
    let old = &document.clip[index];
    let end = old.get_ref().values().map(|field| field.span().end).max().unwrap_or(old.span().end);
    Ok(old.span().start..end)
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

fn with_modes(mut text: String, agent: Agent, modes: &[String]) -> Result<String, Error> {
    #[derive(Deserialize)]
    struct Document {
        modes: Option<toml::Spanned<BTreeMap<String, toml::Spanned<toml::Value>>>>,
    }

    let document: Document = toml::from_str(&text)?;
    let value = toml::Value::try_from(modes)?.to_string();
    match document.modes {
        Some(table) => match table.get_ref().get(agent.name()) {
            Some(old) => text.replace_range(old.span(), &value),
            None if text[table.span()].starts_with('[') => text.insert_str(table.span().end, &format!("\n{} = {value}", agent.name())),
            None => return Err("Add these modes to config.toml by hand; its modes are not a [modes] table".into()),
        },
        None => text.push_str(&format!("\n[modes]\n{} = {value}\n", agent.name())),
    }
    Config::parse(&text)?;
    Ok(text)
}

fn with_host_name(text: String, name: Option<&str>) -> Result<String, Error> {
    with_top_level(text, "name", name)
}

fn with_top_level(mut text: String, key: &str, value: Option<&str>) -> Result<String, Error> {
    let document: BTreeMap<toml::Spanned<String>, toml::Spanned<toml::Value>> = toml::from_str(&text)?;
    let old = document.iter().find(|(name, _)| name.get_ref() == key).map(|(_, old)| old.span());
    let value = value.map(|value| toml::Value::String(value.to_owned()).to_string());
    match (old, value) {
        (Some(old), Some(value)) => text.replace_range(old, &value),
        (None, Some(value)) => text.insert_str(0, &format!("{key} = {value}\n")),
        (Some(old), None) => {
            let start = text[..old.start].rfind('\n').map_or(0, |newline| newline + 1);
            let end = text[old.end..].find('\n').map_or(text.len(), |newline| old.end + newline + 1);
            text.replace_range(start..end, "");
        }
        (None, None) => {}
    }
    Config::parse(&text)?;
    Ok(text)
}

fn theme<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Theme, D::Error> {
    Theme::resolve(&String::deserialize(deserializer)?).map_err(D::Error::custom)
}

fn rigs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Rig>, D::Error> {
    let rigs = Vec::<toml::Value>::deserialize(deserializer)?.into_iter().map(|mut value| {
        if let Some(fields) = value.as_table_mut() {
            for field in ["command", "args", "resume"] {
                fields.remove(field);
            }
        }
        Rig::deserialize(value).map_err(D::Error::custom)
    }).collect::<Result<Vec<_>, _>>()?;
    let mut names = HashSet::new();
    if let Some(rig) = rigs.iter().find(|rig| !names.insert(&rig.name)) {
        return Err(D::Error::custom(format!("more than one rig is named {:?}", rig.name)));
    }
    for rig in &rigs {
        let modes = rig.agent.modes();
        if let Some(mode) = &rig.mode
            && !modes.contains(&mode.as_str())
        {
            let (name, agent, modes) = (&rig.name, rig.agent.name(), modes.join(", "));
            return Err(D::Error::custom(format!("rig {name:?}: {agent} has no mode {mode:?}; its modes are {modes}")));
        }
    }
    Ok(rigs)
}

fn modes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<Agent, Vec<String>>, D::Error> {
    let modes = BTreeMap::<Agent, Vec<String>>::deserialize(deserializer)?;
    for (agent, chosen) in &modes {
        let known = agent.chat_modes();
        if chosen.is_empty() {
            return Err(D::Error::custom(format!("modes.{} needs at least one mode; its modes are {}", agent.name(), known.join(", "))));
        }
        if let Some(mode) = chosen.iter().find(|mode| !known.contains(&mode.as_str())) {
            return Err(D::Error::custom(format!("modes.{}: no mode {mode:?}; its modes are {}", agent.name(), known.join(", "))));
        }
    }
    Ok(modes)
}

fn clips<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Clip>, D::Error> {
    let clips = Vec::<Clip>::deserialize(deserializer)?;
    let mut names = HashSet::new();
    for clip in &clips {
        clip.check().map_err(|problem| D::Error::custom(format!("clip {:?}: {problem}", clip.name)))?;
        if !names.insert(&clip.name) {
            return Err(D::Error::custom(format!("more than one clip is named {:?}", clip.name)));
        }
    }
    Ok(clips)
}

fn fraction<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let share = f64::deserialize(deserializer)?;
    if share > 0.0 && share <= 1.0 {
        Ok(share)
    } else {
        Err(D::Error::custom(format!("{share} is not a share of a Usage window; use one greater than 0 and at most 1, such as 0.9")))
    }
}

fn faces<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Face>, D::Error> {
    let files = Vec::<PathBuf>::deserialize(deserializer)?;
    files.into_iter().enumerate()
        .map(|(index, path)| face(expand(path), format!("/fonts/host/{index}")))
        .collect::<Result<_, _>>().map_err(D::Error::custom)
}

pub fn expand(path: PathBuf) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => crate::home().join(rest),
        Err(_) => path,
    }
}

/// Each face file's weight and style, by its path, modification time and length.
static FACES: Mutex<BTreeMap<(PathBuf, Option<SystemTime>, u64), (String, &'static str)>> = Mutex::new(BTreeMap::new());

pub fn face(path: PathBuf, url: String) -> Result<Face, Error> {
    let metadata = fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let key = (path, metadata.modified().ok(), metadata.len());
    if let Some((weight, style)) = FACES.lock().unwrap().get(&key).cloned() {
        return Ok(Face { path: key.0, url, weight, style });
    }
    let data = fs::read(&key.0).map_err(|error| format!("{}: {error}", key.0.display()))?;
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
    let path = key.0.clone();
    FACES.lock().unwrap().insert(key, (weight.clone(), style));
    Ok(Face { path, url, weight, style })
}

pub fn watch(path: &Path) -> io::Result<watch::Receiver<Arc<Config>>> {
    let path = std::path::absolute(path)?;
    let config = Config::seed_rigs(&path).and_then(|()| Config::load(&path)).unwrap_or_else(|error| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_rigs_are_saved_once_and_editable_like_other_rigs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new/config.toml");
        Config::seed_rigs(&path).unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(config.rigs, [Rig::starter(Agent::ClaudeCode), Rig::starter(Agent::Codex)]);
        let mut rig = config.rigs[0].clone();
        rig.name = "Writer".to_owned();
        rig.model = Some("sonnet".to_owned());
        rig.effort = Some("low".to_owned());
        rig.mode = Some("plan".to_owned());
        write_rig(&path, Some("Claude Code"), rig.clone()).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        Config::seed_rigs(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        assert_eq!(Config::load(&path).unwrap().rigs, [rig, config.rigs[1].clone()]);
    }

    #[test]
    fn starter_rigs_preserve_existing_configuration_and_explicit_rig_lists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "# Mine\nroots = []\n[editor]\nkeymap = 'vim'\n";
        fs::write(&path, original).unwrap();
        Config::seed_rigs(&path).unwrap();
        assert!(fs::read_to_string(&path).unwrap().starts_with(original));
        assert_eq!(Config::load(&path).unwrap().rigs.len(), 2);
        for text in ["rig = []\n", "[[rig]]\nname = 'Mine'\nagent = 'codex'\n"] {
            fs::write(&path, text).unwrap();
            Config::seed_rigs(&path).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
        fs::write(&path, "[invalid").unwrap();
        assert!(Config::seed_rigs(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "[invalid");
    }

    #[test]
    fn storage_limits_have_defaults_and_can_be_configured() {
        assert_eq!(Config::parse("").unwrap().storage, Storage::default());
        let config = Config::parse("[storage]\nmin_free_gib = 4\ncache_max_gib = 12\ncache_max_age_days = 3\n").unwrap();
        assert_eq!(config.storage, Storage { min_free_gib: 4, cache_max_gib: 12, cache_max_age_days: 3 });
        assert!(check("[storage]\nmin_free_gib = -1\n").is_some());
    }

    #[test]
    fn rig_editor_preserves_other_settings_and_private_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "# Keep this\nroots = []\n\n[[rig]]\nname = 'Mine'\nagent = 'codex'\nshortcut = 'c'\nenv = { CODEX_HOME = '/tmp/alternate' }\n\n[editor]\nkeymap = 'vim'\n";
        fs::write(&path, original).unwrap();
        let mut rig = Config::parse(original).unwrap().rigs.remove(0);
        rig.model = Some("a-model".to_owned());
        rig.effort = Some("high".to_owned());
        rig.mode = Some("default".to_owned());
        rig.env.clear();
        rig.shortcut = None;
        rig.name = " Reviewer ".to_owned();
        let saved = write_rig(&path, Some("Mine"), rig).unwrap();
        assert_eq!(saved[0].name, "Reviewer");
        assert_eq!(saved[0].env["CODEX_HOME"], "/tmp/alternate");
        assert_eq!(saved[0].shortcut.as_deref(), Some("c"));
        assert_eq!(saved[0].model.as_deref(), Some("a-model"));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# Keep this\nroots = []"), "{text}");
        assert!(text.contains("[editor]\nkeymap = 'vim'"), "{text}");
        let mut new = saved[0].clone();
        new.name = "Another".to_owned();
        let rigs = write_rig(&path, None, new.clone()).unwrap();
        assert_eq!(rigs.len(), 2);
        assert!(write_rig(&path, None, new).is_err());
        let before = fs::read_to_string(&path).unwrap();
        let mut invalid = rigs[1].clone();
        invalid.mode = Some("invalid".to_owned());
        assert!(write_rig(&path, Some("Another"), invalid).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        for original in [None, Some("Reviewer")] {
            for name in ["", "   ", "Another"] {
                let mut invalid = saved[0].clone();
                invalid.name = name.to_owned();
                assert!(write_rig(&path, original, invalid).is_err());
                assert_eq!(fs::read_to_string(&path).unwrap(), before);
            }
        }
    }

    #[test]
    fn clips_keep_multiline_text_and_other_settings_through_saves_reorders_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "# Keep this\nroots = []\n\n[editor]\nkeymap = 'vim'\n";
        fs::write(&path, original).unwrap();
        let verify = Clip { name: "verify".to_owned(), text: "Run the tests.\n\nThen commit with a \"short\" message.\n".to_owned() };
        let review = Clip { name: "review".to_owned(), text: "Review the diff.".to_owned() };
        write_clip(&path, None, verify.clone()).unwrap();
        assert_eq!(write_clip(&path, None, review.clone()).unwrap(), [verify.clone(), review.clone()]);
        let edited = Clip { name: "check".to_owned(), text: "Check it.\nTwice.".to_owned() };
        assert_eq!(write_clip(&path, Some("verify"), edited.clone()).unwrap(), [edited.clone(), review.clone()]);
        assert_eq!(reorder_clips(&path, &["review".to_owned(), "check".to_owned()]).unwrap(), [review.clone(), edited.clone()]);
        assert_eq!(delete_clip(&path, "review").unwrap(), [edited.clone()]);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(original), "{text}");
        assert_eq!(Config::parse(&text).unwrap().clips, [edited]);
    }

    #[test]
    fn clips_refuse_clashing_or_unusable_names_and_empty_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_clip(&path, None, Clip { name: " verify ".to_owned(), text: "Run the tests.".to_owned() }).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        for (name, text) in [("verify", "Again."), ("", "Text."), ("two words", "Text."), ("empty", "  \n")] {
            assert!(write_clip(&path, None, Clip { name: name.to_owned(), text: text.to_owned() }).is_err());
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        assert_eq!(Config::parse(&before).unwrap().clips[0].name, "verify");
        assert!(check("[[clip]]\nname = 'a'\ntext = 'x'\n[[clip]]\nname = 'a'\ntext = 'y'\n").is_some());
        assert!(check("[[clip]]\nname = 'a b'\ntext = 'x'\n").is_some());
    }

    #[test]
    fn rig_order_preserves_settings_and_rejects_stale_lists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        for environment in ["env = { CODEX_HOME = '/tmp/alternate' }\n", "[rig.env]\nCODEX_HOME = '/tmp/alternate'\n", "[rig.env]\n"] {
            let original = format!("# Keep this\nroots = []\n[[rig]]\nname = 'First'\nagent = 'codex'\nshortcut = 'c'\n{environment}\n[[rig]]\nname = 'Second'\nagent = 'claude-code'\nmodel = 'opus'\n\n[editor]\nkeymap = 'vim'\n");
            fs::write(&path, &original).unwrap();
            let before = Config::load(&path).unwrap().rigs;
            let order = vec!["Second".to_owned(), "First".to_owned()];
            let after = reorder_rigs(&path, &order).unwrap();
            assert_eq!(after, [before[1].clone(), before[0].clone()]);
            assert_eq!(Config::load(&path).unwrap().rigs, after);
            let saved = fs::read_to_string(&path).unwrap();
            assert!(saved.starts_with("# Keep this\nroots = []"));
            assert!(saved.contains("[editor]\nkeymap = 'vim'"));
            for order in [vec!["First"], vec!["First", "First"], vec!["First", "Missing"], vec!["First", "Second", "Extra"]] {
                assert!(reorder_rigs(&path, &order.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
                assert_eq!(fs::read_to_string(&path).unwrap(), saved);
            }
        }
    }

    #[test]
    fn deleting_rigs_preserves_settings_and_keeps_an_empty_list_on_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        for environment in ["env = { CODEX_HOME = '/tmp/alternate' }\n", "[rig.env]\nCODEX_HOME = '/tmp/alternate'\n", "[rig.env]\n"] {
            let original = format!("# Keep this\nroots = []\n[[rig]]\nname = 'First'\nagent = 'codex'\n{environment}\n[[rig]]\nname = 'Second'\nagent = 'claude-code'\n\n[editor]\nkeymap = 'vim'\n");
            for first in ["First", "Second"] {
                fs::write(&path, &original).unwrap();
                let remaining = delete_rig(&path, first).unwrap();
                assert_eq!(remaining.len(), 1);
                assert_ne!(remaining[0].name, first);
                let saved = fs::read_to_string(&path).unwrap();
                assert!(saved.contains("# Keep this\nroots = []"), "{saved}");
                assert!(saved.contains("[editor]\nkeymap = 'vim'"), "{saved}");
                assert!(delete_rig(&path, first).is_err());
                assert_eq!(fs::read_to_string(&path).unwrap(), saved);
                assert!(delete_rig(&path, &remaining[0].name).unwrap().is_empty());
                let saved = fs::read_to_string(&path).unwrap();
                Config::seed_rigs(&path).unwrap();
                assert_eq!(fs::read_to_string(&path).unwrap(), saved);
                assert!(Config::load(&path).unwrap().rigs.is_empty());
                let added = Rig::starter(Agent::Codex);
                assert_eq!(write_rig(&path, None, added.clone()).unwrap(), [added]);
            }
        }
    }

    #[test]
    fn available_modes_default_to_all_and_keep_the_agents_order() {
        let config = Config::parse("[modes]\nclaude-code = ['plan', 'default']\n").unwrap();
        assert_eq!(config.available_modes(Agent::ClaudeCode), ["default", "plan"]);
        assert_eq!(config.available_modes(Agent::Codex), ["read-only", "default", "approve-for-me", "full-access", "plan"]);
        for text in ["[modes]\ncodex = []\n", "[modes]\ncodex = ['acceptEdits']\n", "[modes]\ncursor = ['default']\n"] {
            assert!(check(text).is_some(), "{text}");
        }
    }

    #[test]
    fn mode_writes_keep_the_rest_of_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "# Mine\nroots = []\n[[rig]]\nname = 'Claude'\nagent = 'claude-code'\n";
        fs::write(&path, original).unwrap();
        let chosen = |modes: &[&str]| modes.iter().map(|mode| mode.to_string()).collect::<Vec<_>>();
        write_modes(&path, Agent::ClaudeCode, &chosen(&["default", "plan"])).unwrap();
        write_modes(&path, Agent::Codex, &chosen(&["plan"])).unwrap();
        let config = write_modes(&path, Agent::ClaudeCode, &chosen(&["acceptEdits"])).unwrap();
        assert_eq!(config.available_modes(Agent::ClaudeCode), ["acceptEdits"]);
        assert_eq!(config.available_modes(Agent::Codex), ["plan"]);
        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(saved, format!("{original}\n[modes]\ncodex = [\"plan\"]\nclaude-code = [\"acceptEdits\"]\n"));
        assert!(write_modes(&path, Agent::Codex, &[]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        fs::write(&path, "modes = { codex = ['plan'] }\n").unwrap();
        assert!(write_modes(&path, Agent::ClaudeCode, &chosen(&["plan"])).is_err());
        assert_eq!(write_modes(&path, Agent::Codex, &chosen(&["default"])).unwrap().available_modes(Agent::Codex), ["default"]);
    }

    #[test]
    fn chat_only_rigs_accept_old_configs_without_accepting_unknown_fields() {
        let text = "[[rig]]\nname = 'Claude'\nagent = 'claude-code'\n";
        let rig = Config::parse(text).unwrap().rigs.remove(0);
        let legacy = format!("{text}command = 'unused'\nargs = ['ignored']\nresume = 'unused {{session}}'\n");
        assert_eq!(Config::parse(&legacy).unwrap().rigs, [rig]);
        assert!(check(&format!("{text}modle = 'opus'\n")).is_some());
    }

    #[test]
    fn configured_rigs_use_the_same_permission_defaults_as_fallback_rigs() {
        let defaults = Config::default();
        for agent in [Agent::ClaudeCode, Agent::Codex, Agent::Copilot] {
            let config = Config::parse(&format!("[[rig]]\nname = 'Mine'\nagent = '{}'\n", agent.name())).unwrap();
            assert_eq!(config.rigs[0].settings(), defaults.rig_for(agent).settings());
        }
    }

    #[test]
    fn rigs_carry_model_effort_and_mode() {
        let config = Config::parse(
            r#"
[[rig]]
name = "Opus"
agent = "claude-code"
command = "claude"
env = { CLAUDE_CONFIG_DIR = "~/.claude-alt" }
model = "opus"
effort = "xhigh"
mode = "acceptEdits"

[[rig]]
name = "Codex"
agent = "codex"
command = "codex"
mode = "full-access"

[[rig]]
name = "Plain"
agent = "codex"
command = "codex"
"#,
        )
        .unwrap();
        let settings: Vec<AgentSettings> = config.rigs.iter().map(Rig::settings).collect();
        let text = |value: &str| Some(value.to_owned());
        assert_eq!(
            settings,
            [
                AgentSettings { model: text("opus"), effort: text("xhigh"), mode: text("acceptEdits") },
                AgentSettings { mode: text("full-access"), ..AgentSettings::default() },
                AgentSettings { mode: text("approve-for-me"), ..AgentSettings::default() },
            ]
        );
        assert_eq!(config.rigs[0].env["CLAUDE_CONFIG_DIR"], "~/.claude-alt");
    }

    #[test]
    fn a_mode_its_agent_lacks_is_an_error_naming_the_rig() {
        for (agent, mode) in [("codex", "acceptEdits"), ("codex", "plan"), ("claude-code", "read-only"), ("claude-code", "yolo")] {
            let text = format!("roots = []\n\n[[rig]]\nname = \"Mine\"\nagent = \"{agent}\"\ncommand = \"x\"\nmode = \"{mode}\"\n");
            let invalid = check(&text).unwrap_or_else(|| panic!("{agent} accepted mode {mode}"));
            assert!(invalid.message.starts_with(&format!("rig \"Mine\": {agent} has no mode \"{mode}\"")), "{}", invalid.message);
            assert_eq!(invalid.line, Some(3));
        }
        for (agent, mode) in [("codex", "read-only"), ("codex", "default"), ("codex", "approve-for-me"), ("claude-code", "default"), ("claude-code", "plan")] {
            let text = format!("[[rig]]\nname = \"Mine\"\nagent = \"{agent}\"\ncommand = \"x\"\nmode = \"{mode}\"\n");
            assert!(check(&text).is_none(), "{agent} refused mode {mode}");
        }
    }

    #[test]
    fn a_run_pauses_at_the_share_of_a_usage_window_the_loop_table_sets() {
        assert_eq!(Config::parse("").unwrap().pause_at, 0.9);
        assert_eq!(Config::parse("[loop]\npause_at = 0.75\n").unwrap().pause_at, 0.75);
        assert_eq!(Config::parse("[loop]\npause_at = 1\n").unwrap().pause_at, 1.0);
        let invalid = check("roots = []\n[loop]\npause_at = 90\n").unwrap();
        assert!(invalid.message.contains("90 is not a share of a Usage window"), "{}", invalid.message);
        assert_eq!(invalid.line, Some(3));
        assert!(check("[loop]\npause = 0.5\n").is_some());
    }
}
