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
    r#loop: Loop,
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
            r#loop: Loop::default(),
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
    #[serde(default, skip_serializing)]
    pub model: Option<String>,
    #[serde(default, skip_serializing)]
    pub effort: Option<String>,
    #[serde(default, skip_serializing)]
    pub mode: Option<String>,
}

impl Rig {
    pub fn settings(&self) -> AgentSettings {
        let mode = self.mode.clone().or_else(|| (self.agent == Agent::Codex).then(|| "approve-for-me".to_owned()));
        AgentSettings { model: self.model.clone(), effort: self.effort.clone(), mode }
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

    /// The modes a Rig can start this Agent in, by the Agent's own names.
    pub fn modes(self) -> &'static [&'static str] {
        match self {
            Agent::ClaudeCode => &["default", "acceptEdits", "plan", "auto", "dontAsk", "bypassPermissions"],
            Agent::Codex => &["read-only", "default", "approve-for-me", "full-access"],
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
    pub pause_at: f64,
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
            pause_at: Loop::default().pause_at,
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
            r#loop,
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
            pause_at: r#loop.pause_at,
        })
    }

    /// The first Rig for an Agent, or the Agent's defaults when no Rig runs it.
    pub fn rig_for(&self, agent: Agent) -> Rig {
        if let Some(rig) = self.rigs.iter().find(|rig| rig.agent == agent) {
            return rig.clone();
        }
        let (name, mode) = match agent {
            Agent::ClaudeCode => ("Claude Code", "auto"),
            Agent::Codex => ("Codex", "approve-for-me"),
        };
        Rig {
            name: name.to_owned(),
            agent,
            env: BTreeMap::new(),
            shortcut: None,
            model: None,
            effort: None,
            mode: Some(mode.to_owned()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_only_rigs_accept_old_configs_without_accepting_unknown_fields() {
        let text = "[[rig]]\nname = 'Claude'\nagent = 'claude-code'\n";
        let rig = Config::parse(text).unwrap().rigs.remove(0);
        let legacy = format!("{text}command = 'unused'\nargs = ['ignored']\nresume = 'unused {{session}}'\n");
        assert_eq!(Config::parse(&legacy).unwrap().rigs, [rig]);
        assert!(check(&format!("{text}modle = 'opus'\n")).is_some());
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
