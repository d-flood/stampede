use std::{collections::BTreeMap, io, path::Path, process::Stdio, time::Duration};

use nix::{sys::signal::{Signal, killpg}, unistd::Pid};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::{mpsc::UnboundedSender, oneshot},
    time::timeout,
};

const HEAD: usize = 10_000;
const TAIL: usize = 20_000;
pub const SHOWN: usize = 1 << 20;

pub enum Progress {
    Output { index: usize, text: String, error: bool },
    Exit { index: usize, code: Option<i32> },
}

/// Runs a user's `!` command; dropping or firing the returned sender stops its whole process group.
pub fn spawn(index: usize, command: &str, cwd: &Path, env: &BTreeMap<String, String>, progress: UnboundedSender<Progress>) -> io::Result<oneshot::Sender<()>> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    let mut child = Command::new(shell)
        .arg("-lc").arg(command).current_dir(cwd).envs(env)
        .envs([("PAGER", "cat"), ("GIT_PAGER", "cat"), ("GIT_TERMINAL_PROMPT", "0")])
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .process_group(0).kill_on_drop(true).spawn()?;
    let group = child.id().map(|pid| Pid::from_raw(pid as i32));
    let readers = [
        tokio::spawn(read(child.stdout.take(), index, false, progress.clone())),
        tokio::spawn(read(child.stderr.take(), index, true, progress.clone())),
    ];
    let (stop, stopped) = oneshot::channel();
    tokio::spawn(async move {
        let code = tokio::select! {
            status = child.wait() => status.ok().and_then(|status| status.code()),
            _ = stopped => {
                for signal in [Signal::SIGTERM, Signal::SIGKILL] {
                    if let Some(group) = group { let _ = killpg(group, signal); }
                    if timeout(Duration::from_secs(2), child.wait()).await.is_ok() { break; }
                }
                None
            }
        };
        for reader in readers {
            let abort = reader.abort_handle();
            if timeout(Duration::from_secs(1), reader).await.is_err() { abort.abort(); }
        }
        let _ = progress.send(Progress::Exit { index, code });
    });
    Ok(stop)
}

async fn read(pipe: Option<impl AsyncRead + Unpin>, index: usize, error: bool, progress: UnboundedSender<Progress>) {
    let Some(mut pipe) = pipe else { return };
    let mut buffer = vec![0; 8192];
    let mut carry = Vec::new();
    while let Ok(read) = pipe.read(&mut buffer).await && read > 0 {
        carry.extend_from_slice(&buffer[..read]);
        let valid = match std::str::from_utf8(&carry) {
            Err(invalid) if invalid.error_len().is_none() => invalid.valid_up_to(),
            _ => carry.len(),
        };
        let text = String::from_utf8_lossy(&carry[..valid]).into_owned();
        carry.drain(..valid);
        if !text.is_empty() && progress.send(Progress::Output { index, text, error }).is_err() { return; }
    }
    if !carry.is_empty() {
        let _ = progress.send(Progress::Output { index, text: String::from_utf8_lossy(&carry).into_owned(), error });
    }
}

#[derive(Default)]
pub struct Capture {
    head: String,
    tail: String,
    omitted: usize,
}

impl Capture {
    pub fn push(&mut self, mut text: &str) {
        if self.head.len() < HEAD {
            let at = boundary(text, HEAD - self.head.len());
            self.head.push_str(&text[..at]);
            text = &text[at..];
        }
        self.tail.push_str(text);
        if self.tail.len() > 2 * TAIL {
            let cut = boundary(&self.tail, self.tail.len() - TAIL);
            self.omitted += self.tail[..cut].matches('\n').count();
            self.tail.drain(..cut);
        }
    }

    pub fn text(&self) -> String {
        let cut = boundary(&self.tail, self.tail.len().saturating_sub(TAIL));
        if cut == 0 && self.omitted == 0 {
            return format!("{}{}", self.head, self.tail);
        }
        let omitted = self.omitted + self.tail[..cut].matches('\n').count();
        format!("{}\n… {omitted} lines omitted …\n{}", self.head, &self.tail[cut..])
    }
}

fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) { at -= 1; }
    at
}

pub fn block(command: &str, stdout: &str, stderr: &str, code: Option<i32>) -> String {
    let exit = match code {
        Some(0) => String::new(),
        Some(code) => format!("<bash-exit-code>{code}</bash-exit-code>"),
        None => "<bash-exit-code>stopped</bash-exit-code>".to_owned(),
    };
    format!("<bash-input>{}</bash-input>\n<bash-stdout>{}</bash-stdout><bash-stderr>{}</bash-stderr>{exit}", escape(command), escape(stdout), escape(stderr))
}

#[derive(Debug, PartialEq)]
pub enum Part {
    Input(String),
    Output(String),
    Exit,
}

pub fn parse(text: &str) -> (Vec<Part>, &str) {
    let mut parts = Vec::new();
    let mut rest = text;
    loop {
        let trimmed = rest.trim_start();
        let Some(tag) = ["bash-input", "bash-stdout", "bash-stderr", "bash-exit-code"].into_iter().find(|tag| trimmed.starts_with(&format!("<{tag}>"))) else { break };
        let close = format!("</{tag}>");
        let Some(end) = trimmed.find(&close) else { break };
        let inner = unescape(&trimmed[tag.len() + 2..end]);
        parts.push(match tag {
            "bash-input" => Part::Input(inner.trim().to_owned()),
            "bash-exit-code" => Part::Exit,
            _ => Part::Output(inner),
        });
        rest = &trimmed[end + close.len()..];
    }
    if parts.is_empty() { (parts, text) } else { (parts, rest.trim_start()) }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_round_trip_with_markup_and_the_typed_text_after_them() {
        let first = block("grep '<a>' x | head", "<a> & b\n", "", Some(0));
        let second = block("false", "", "nope\n", Some(1));
        let prompt = format!("{first}\n{second}\n\nWhat failed?");
        let (parts, rest) = parse(&prompt);
        assert_eq!(parts, [
            Part::Input("grep '<a>' x | head".to_owned()), Part::Output("<a> & b\n".to_owned()), Part::Output(String::new()),
            Part::Input("false".to_owned()), Part::Output(String::new()), Part::Output("nope\n".to_owned()), Part::Exit,
        ]);
        assert_eq!(rest, "What failed?");
        assert_eq!(parse("Plain <bash-input> mention"), (Vec::new(), "Plain <bash-input> mention"));
    }

    #[test]
    fn claude_code_records_input_and_output_as_separate_prompts() {
        assert_eq!(parse("<bash-input> gh auth status</bash-input>").0, [Part::Input("gh auth status".to_owned())]);
        let (parts, rest) = parse("<bash-stdout>{&quot;id&quot;:1} &gt; 0</bash-stdout><bash-stderr></bash-stderr>");
        assert_eq!((parts, rest), (vec![Part::Output("{&quot;id&quot;:1} > 0".to_owned()), Part::Output(String::new())], ""));
    }

    #[test]
    fn captures_keep_the_head_and_tail_and_count_the_lines_between() {
        let mut capture = Capture::default();
        capture.push("short\n");
        assert_eq!(capture.text(), "short\n");
        let mut capture = Capture::default();
        for line in 0..20_000 { capture.push(&format!("line {line:05}\n")); }
        let text = capture.text();
        assert!(text.starts_with("line 00000\n"));
        assert!(text.ends_with("line 19999\n"));
        assert!(text.len() < HEAD + TAIL + 100);
        assert!(text.contains(" lines omitted …\n"));
    }
}
