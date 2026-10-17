use std::{fs, path::Path, process::Command};

fn help(words: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_oikonomos")).args(words).arg("--help").env_clear().output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "oikonomos {} --help failed: {stderr}", words.join(" "));
    String::from_utf8(output.stdout).unwrap()
}

fn subcommands(help: &str) -> Vec<String> {
    let listed = help.lines().skip_while(|line| *line != "Commands:").skip(1).take_while(|line| line.starts_with("  "));
    listed.filter_map(|line| line.split_whitespace().next()).map(str::to_owned).collect()
}

/// The words of a command written after `oikonomos `, up to the first argument or the end of its code span.
fn command(mention: &str) -> Vec<&str> {
    let mut words = Vec::new();
    for token in mention.split_whitespace() {
        let word = token.trim_end_matches(['`', '.', ',', ';', ':', ')', '"', '\'']);
        if word.is_empty() || !word.starts_with(|c: char| c.is_ascii_lowercase()) || !word.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            break;
        }
        words.push(word);
        if word.len() < token.len() {
            break;
        }
    }
    words
}

#[test]
fn every_template_names_only_oikonomos_subcommands_that_exist() {
    let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/templates");
    let mut checked = 0;
    for entry in fs::read_dir(&templates).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        for mention in text.split("oikonomos ").skip(1) {
            let words = command(mention);
            for depth in 0..words.len() {
                let known = subcommands(&help(&words[..depth]));
                if known.is_empty() {
                    break;
                }
                let named = words[..=depth].join(" ");
                assert!(known.iter().any(|known| known == words[depth]), "{} names `oikonomos {named}`, which is not a command", path.display());
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "no Template in {} names an oikonomos command", templates.display());
}
