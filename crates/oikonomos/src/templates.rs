use std::{borrow::Cow, path::Path};

use serde::Serialize;

#[derive(Serialize)]
pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    #[serde(skip)]
    text: &'static str,
}

pub const TEMPLATES: &[Template] = &[
    Template { name: "grill", description: "interview me about a plan, keeping the glossary and ADRs", text: include_str!("templates/grill.md") },
    Template { name: "spec", description: "write the Brief and create the Epic", text: include_str!("templates/spec.md") },
    Template { name: "tickets", description: "write the Epic's Tickets and its Setup Command", text: include_str!("templates/tickets.md") },
];

/// The first prompt of every Ticket's Chat. The loop sends it, so the slash menu does not offer it.
pub const TICKET: Template = Template { name: "ticket", description: "work one Ticket", text: include_str!("templates/ticket.md") };

impl Template {
    /// The CLI only finds a server on a non-default data directory when told it.
    pub fn opening(&self, data_dir: &Path) -> String {
        let text = self.text.trim_end();
        if data_dir == crate::data_dir() {
            return text.to_owned();
        }
        let dir = data_dir.display();
        format!("{text}\n\nThis Oikonomos server keeps its data in {dir}, so pass `--data-dir {dir}` to every `oikonomos` command.")
    }
}

pub fn expand<'a>(text: &'a str, data_dir: &Path) -> Cow<'a, str> {
    let Some(named) = text.strip_prefix('/') else { return Cow::Borrowed(text) };
    let (name, typed) = named.split_once(char::is_whitespace).unwrap_or((named, ""));
    match (TEMPLATES.iter().find(|template| template.name == name), typed.trim()) {
        (None, _) => Cow::Borrowed(text),
        (Some(template), "") => Cow::Owned(template.opening(data_dir)),
        (Some(template), typed) => Cow::Owned(format!("{}\n\n{typed}", template.opening(data_dir))),
    }
}

/// The Template a sent text was expanded from, and the text as it was typed.
pub fn recognize(text: &str, data_dir: &Path) -> Option<(&'static str, String)> {
    TEMPLATES.iter().chain([&TICKET]).find_map(|template| {
        let typed = text.strip_prefix(&template.opening(data_dir))?;
        Some(match typed {
            "" => (template.name, format!("/{}", template.name)),
            typed => (template.name, format!("/{} {}", template.name, typed.strip_prefix("\n\n")?)),
        })
    })
}
