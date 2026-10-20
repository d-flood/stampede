use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::PathBuf,
    process::Command,
};

use crate::config::{Font, face};

const LOADABLE: [&str; 4] = ["ttf", "otf", "woff", "woff2"];

fn installed() -> io::Result<BTreeMap<String, BTreeSet<PathBuf>>> {
    let output = match Command::new("fc-list").args(["--format", "%{family[0]}\t%{file}\n"]).output() {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        output => output?,
    };
    let mut families = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((family, file)) = line.split_once('\t') else { continue };
        let path = PathBuf::from(file);
        let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_ascii_lowercase();
        if !family.is_empty() && LOADABLE.contains(&extension.as_str()) {
            families.entry(family.to_owned()).or_default().insert(path);
        }
    }
    Ok(families)
}

pub fn families() -> io::Result<Vec<String>> {
    Ok(installed()?.into_keys().collect())
}

/// A family's faces, which are empty when the family is not installed.
pub fn resolve(family: &str) -> io::Result<Font> {
    let segment: String = family.bytes().map(|byte| match byte {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => char::from(byte).to_string(),
        _ => format!("%{byte:02X}"),
    }).collect();
    let files = installed()?.remove(family).unwrap_or_default();
    let faces = files.into_iter().enumerate()
        .filter_map(|(index, path)| face(path, format!("/fonts/system/{segment}/{index}")).ok())
        .collect();
    Ok(Font { family: family.to_owned(), faces })
}

pub fn file(family: &str, index: usize) -> io::Result<Option<PathBuf>> {
    Ok(installed()?.remove(family).and_then(|files| files.into_iter().nth(index)))
}
