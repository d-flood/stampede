use std::{collections::BTreeSet, fs, io};

use rust_embed::RustEmbed;
use serde::Serialize;

use crate::config::Error;

#[derive(RustEmbed)]
#[folder = "vendor/ghostty-themes/themes"]
struct Bundled;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub background: String,
    pub foreground: String,
    pub palette: [String; 16],
    pub cursor: String,
    pub cursor_text: String,
    pub selection_background: String,
    pub selection_foreground: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Theme {
    pub name: Option<String>,
    pub light: Palette,
    pub dark: Palette,
}

impl Theme {
    pub fn resolve(spec: &str) -> Result<Self, Error> {
        let name = Some(spec.to_owned());
        let pair: Option<Vec<_>> = spec.split(',').map(|part| part.split_once(':')).collect();
        let Some(pair) = pair else {
            let palette = lookup(spec.trim())?;
            return Ok(Self { name, light: palette.clone(), dark: palette });
        };
        let named = |scheme: &str| {
            pair.iter()
                .find(|(key, _)| key.trim() == scheme)
                .map(|(_, name)| lookup(name.trim()))
                .ok_or_else(|| format!("theme {spec:?} names no {scheme} theme"))
        };
        Ok(Self { name, light: named("light")??, dark: named("dark")?? })
    }
}

impl Default for Theme {
    fn default() -> Self {
        let palette = parse("").unwrap();
        Self { name: None, light: palette.clone(), dark: palette }
    }
}

pub fn names() -> io::Result<Vec<String>> {
    let mut names: BTreeSet<_> = Bundled::iter().map(|name| name.into_owned()).collect();
    let user = crate::home().join(".config/ghostty/themes");
    match fs::read_dir(user) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                if entry.path().is_file() {
                    if let Some(name) = entry.file_name().to_str().filter(|name| !name.starts_with('.')) {
                        names.insert(name.to_owned());
                    }
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(names.into_iter().collect())
}

fn lookup(name: &str) -> Result<Palette, Error> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return Err(format!("invalid theme name {name:?}").into());
    }
    let user = crate::home().join(".config/ghostty/themes").join(name);
    let text = match fs::read_to_string(&user) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let file = Bundled::get(name).ok_or_else(|| format!("no theme named {name:?}"))?;
            String::from_utf8(file.data.into_owned())?
        }
        Err(error) => return Err(format!("{}: {error}", user.display()).into()),
    };
    parse(&text).map_err(|error| format!("theme {name:?}: {error}").into())
}

fn parse(text: &str) -> Result<Palette, Error> {
    let mut palette = [
        "#1d1f21", "#cc6666", "#b5bd68", "#f0c674", "#81a2be", "#b294bb", "#8abeb7", "#c5c8c6",
        "#666666", "#d54e53", "#b9ca4a", "#e7c547", "#7aa6da", "#c397d8", "#70c0b1", "#eaeaea",
    ]
    .map(String::from);
    let mut background = "#282c34".to_owned();
    let mut foreground = "#ffffff".to_owned();
    let [mut cursor, mut cursor_text, mut selection_background, mut selection_foreground] =
        [None, None, None, None];
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key.trim() {
            "background" => background = color(value)?,
            "foreground" => foreground = color(value)?,
            "cursor-color" => cursor = Some(color(value)?),
            "cursor-text" => cursor_text = Some(color(value)?),
            "selection-background" => selection_background = Some(color(value)?),
            "selection-foreground" => selection_foreground = Some(color(value)?),
            "palette" => {
                let (index, value) = value.split_once('=').ok_or("palette needs N=#rrggbb")?;
                let index: usize = index.trim().parse()?;
                if index < 16 {
                    palette[index] = color(value.trim())?;
                }
            }
            _ => {}
        }
    }
    Ok(Palette {
        cursor: cursor.unwrap_or_else(|| foreground.clone()),
        cursor_text: cursor_text.unwrap_or_else(|| background.clone()),
        selection_background: selection_background.unwrap_or_else(|| foreground.clone()),
        selection_foreground: selection_foreground.unwrap_or_else(|| background.clone()),
        background,
        foreground,
        palette,
    })
}

fn color(value: &str) -> Result<String, Error> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(format!("#{}", hex.to_ascii_lowercase()))
    } else {
        Err(format!("{value:?} is not a #rrggbb color").into())
    }
}
