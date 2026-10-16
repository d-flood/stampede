use std::{
    io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::{WalkBuilder, overrides::OverrideBuilder};
use serde::{Deserialize, Serialize};

pub const CAP: usize = 10_000;
const PREVIEW: usize = 200;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub search_id: String,
    pub project_id: String,
    query: String,
    regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    include: Vec<String>,
    exclude: Vec<String>,
    include_ignored: bool,
}

#[derive(Serialize)]
pub struct Found {
    line: u64,
    column: usize,
    preview: String,
}

pub fn run(
    root: &Path,
    query: &Query,
    cancelled: &AtomicBool,
    mut found: impl FnMut(&str, Vec<Found>),
) -> Result<bool, String> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!query.case_sensitive)
        .word(query.whole_word)
        .fixed_strings(!query.regex)
        .line_terminator(Some(b'\n'))
        .build(&query.query)
        .map_err(|error| error.to_string())?;
    let mut globs = OverrideBuilder::new(root);
    for glob in query.include.iter().cloned().chain(query.exclude.iter().map(|glob| format!("!{glob}"))) {
        globs.add(&glob).map_err(|error| error.to_string())?;
    }
    let globs = globs.build().map_err(|error| error.to_string())?;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(0))
        .line_number(true)
        .build();
    // Globs only narrow the walk here; given to the walker as overrides, an
    // include glob would bring back ignored files.
    let walk = WalkBuilder::new(root)
        .standard_filters(!query.include_ignored)
        .hidden(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(move |entry| {
            let dir = entry.file_type().is_some_and(|kind| kind.is_dir());
            entry.file_name() != ".git" && !globs.matched(entry.path(), dir).is_ignore()
        })
        .build();
    let mut total = 0;
    for entry in walk {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Some(path) = entry.path().strip_prefix(root).ok().and_then(Path::to_str) else { continue };
        let mut sink = Collect { matcher: &matcher, cancelled, limit: CAP + 1 - total, matches: Vec::new() };
        if searcher.search_path(&matcher, entry.path(), &mut sink).is_err() || sink.matches.is_empty() {
            continue;
        }
        let mut matches = sink.matches;
        total += matches.len();
        if total > CAP {
            matches.truncate(matches.len() - (total - CAP));
            if !matches.is_empty() {
                found(path, matches);
            }
            return Ok(true);
        }
        found(path, matches);
    }
    Ok(false)
}

struct Collect<'a> {
    matcher: &'a RegexMatcher,
    cancelled: &'a AtomicBool,
    limit: usize,
    matches: Vec<Found>,
}

impl Sink for Collect<'_> {
    type Error = io::Error;

    fn matched(&mut self, _: &Searcher, found: &SinkMatch) -> Result<bool, io::Error> {
        let bytes = found.bytes();
        let line = String::from_utf8_lossy(bytes);
        let line = line.trim_end();
        self.matcher
            .find_iter(bytes, |at| {
                let before = String::from_utf8_lossy(&bytes[..at.start()]);
                let skip = before.chars().count().saturating_sub(PREVIEW / 4);
                let shown: String = line.chars().skip(skip).take(PREVIEW).collect();
                let preview = if skip > 0 { format!("…{shown}") } else { shown.trim_start().to_owned() };
                let column = before.encode_utf16().count() + 1;
                self.matches.push(Found { line: found.line_number().unwrap_or(0), column, preview });
                self.matches.len() < self.limit
            })
            .map_err(io::Error::other)?;
        Ok(self.matches.len() < self.limit && !self.cancelled.load(Ordering::Relaxed))
    }
}
