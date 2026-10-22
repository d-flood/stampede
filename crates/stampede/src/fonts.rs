use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use crate::config::{Font, face};

const LOADABLE: [&str; 4] = ["ttf", "otf", "woff", "woff2"];

type Families = BTreeMap<String, BTreeSet<PathBuf>>;

/// The installed families, with the modification time of each directory that holds them when they were listed.
static INSTALLED: Mutex<Option<(Vec<(PathBuf, Option<SystemTime>)>, Arc<Families>)>> = Mutex::new(None);

fn installed() -> io::Result<Arc<Families>> {
    let mut installed = INSTALLED.lock().unwrap();
    if let Some((directories, families)) = &*installed
        && directories.iter().all(|(directory, at)| modified(directory) == *at)
    {
        return Ok(Arc::clone(families));
    }
    let families = Arc::new(list()?);
    let directories = directories(&families).into_iter().map(|directory| {
        let at = modified(&directory);
        (directory, at)
    });
    *installed = Some((directories.collect(), Arc::clone(&families)));
    Ok(families)
}

fn modified(directory: &Path) -> Option<SystemTime> {
    fs::metadata(directory).and_then(|metadata| metadata.modified()).ok()
}

/// Fontconfig's usual font roots, each directory between them and a listed font, and each font's own directory,
/// so that installing or removing a font anywhere below them changes one of their modification times.
fn directories(families: &Families) -> BTreeSet<PathBuf> {
    let home = crate::home();
    let data = env::var_os("XDG_DATA_HOME").map_or_else(|| home.join(".local/share"), PathBuf::from);
    let roots = [data.join("fonts"), home.join(".fonts"), PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/local/share/fonts")];
    let mut directories = BTreeSet::from_iter(roots.clone());
    for parent in families.values().flatten().filter_map(|file| file.parent()) {
        directories.insert(parent.to_owned());
        if let Some(root) = roots.iter().find(|root| parent.starts_with(root)) {
            directories.extend(parent.ancestors().take_while(|ancestor| ancestor.starts_with(root)).map(Path::to_owned));
        }
    }
    directories
}

fn list() -> io::Result<Families> {
    let output = match Command::new("fc-list").args(["--format", "%{family[0]}\t%{file}\n"]).output() {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        output => output?,
    };
    let mut families = Families::new();
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
    Ok(installed()?.keys().cloned().collect())
}

/// A family's faces, which are empty when the family is not installed.
pub fn resolve(family: &str) -> io::Result<Font> {
    let segment: String = family.bytes().map(|byte| match byte {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => char::from(byte).to_string(),
        _ => format!("%{byte:02X}"),
    }).collect();
    let files = installed()?.get(family).cloned().unwrap_or_default();
    let faces = files.into_iter().enumerate()
        .filter_map(|(index, path)| face(path, format!("/fonts/system/{segment}/{index}")).ok())
        .collect();
    Ok(Font { family: family.to_owned(), faces })
}

pub fn file(family: &str, index: usize) -> io::Result<Option<PathBuf>> {
    Ok(installed()?.get(family).and_then(|files| files.iter().nth(index).cloned()))
}
