use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{io::{AsyncReadExt, BufReader}, net::unix::OwnedReadHalf, sync::oneshot};
use uuid::Uuid;

use super::{Change, Chats, ChoiceDescription, Item, Question, Request, RequestKind, Status};

pub const TOOL: &str = "ask_questions";
pub const CLAUDE_TOOL: &str = "mcp__stampede_chat__ask_questions";
const LIMIT: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Type { Single, Multiple, Text }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub title: String,
    pub asset: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub selected: Vec<String>,
    pub text: String,
    pub skipped: bool,
}

#[derive(Serialize)]
pub struct Reply {
    pub answers: HashMap<String, Response>,
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
}

impl Reply {
    fn cancelled() -> Self { Self { answers: HashMap::new(), cancelled: true, instruction: None } }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input { questions: Vec<Prompt> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prompt {
    id: String,
    header: String,
    question: String,
    #[serde(rename = "type")]
    answer_type: Type,
    #[serde(default)]
    options: Vec<OptionInput>,
    image: Option<ImageInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionInput {
    id: String,
    label: String,
    #[serde(default)]
    description: String,
    preview: Option<String>,
    image: Option<ImageInput>,
    #[serde(default)]
    recommended: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageInput { path: String, title: String }

pub fn tool() -> Value {
    let image = json!({
        "type": "object", "properties": {
            "path": { "type": "string", "description": "Absolute local path to a PNG, JPEG, GIF, or WebP image. It is copied into durable storage." },
            "title": { "type": "string", "minLength": 1, "maxLength": 200, "description": "Accessible image description." },
        }, "required": ["path", "title"], "additionalProperties": false,
    });
    json!({
        "name": TOOL,
        "description": "Prefer this tool for questions in Stampede. Ask independent questions together and wait for answers. Use stable question and option IDs; ask dependent questions later. Types: single, multiple, text. Choices also accept free text. Question/option descriptions accept Markdown; preview is literal text; image uses a local path. Results map IDs to selected options, text and skipped status. Cancellation may include a replacement instruction. Skipping or cancelling is not consent. Total arguments: 256 KiB maximum.",
        "inputSchema": {
            "type": "object", "properties": { "questions": {
                "type": "array", "minItems": 1, "items": {
                    "type": "object", "properties": {
                        "id": { "type": "string", "minLength": 1, "maxLength": 100 },
                        "header": { "type": "string", "minLength": 1, "maxLength": 200 },
                        "question": { "type": "string", "minLength": 1 },
                        "type": { "type": "string", "enum": ["single", "multiple", "text"] },
                        "image": image,
                        "options": { "type": "array", "items": {
                            "type": "object", "properties": {
                                "id": { "type": "string", "minLength": 1, "maxLength": 100 },
                                "label": { "type": "string", "minLength": 1, "maxLength": 200 },
                                "description": { "type": "string" },
                                "preview": { "type": "string", "description": "Literal code or text preview." },
                                "image": image,
                                "recommended": { "type": "boolean" },
                            }, "required": ["id", "label"], "additionalProperties": false,
                        } },
                    }, "required": ["id", "header", "question", "type"], "additionalProperties": false,
                },
            } }, "required": ["questions"], "additionalProperties": false,
        },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false },
    })
}

fn nonempty(value: &str, limit: usize) -> bool { !value.trim().is_empty() && value.chars().count() <= limit }

fn parse(arguments: Value) -> Result<Input, String> {
    if arguments.to_string().len() > LIMIT { return Err("Keep question arguments under 256 KiB.".to_owned()); }
    let input: Input = serde_json::from_value(arguments).map_err(|error| format!("Invalid questions: {error}"))?;
    let mut ids = HashSet::new();
    if input.questions.is_empty() { return Err("Provide at least one question.".to_owned()); }
    for question in &input.questions {
        if !nonempty(&question.id, 100) || !ids.insert(&question.id) || !nonempty(&question.header, 200)
            || question.question.trim().is_empty()
        { return Err("Provide unique question IDs, nonempty headers, and question text.".to_owned()); }
        if (question.answer_type == Type::Text) != question.options.is_empty() {
            return Err("Text questions have no options; choice questions need at least one option.".to_owned());
        }
        let mut options = HashSet::new();
        for option in &question.options {
            if !nonempty(&option.id, 100) || !options.insert(&option.id) || !nonempty(&option.label, 200) {
                return Err("Provide unique option IDs within each question and nonempty labels.".to_owned());
            }
        }
    }
    Ok(input)
}

fn image(chats: &Chats, input: Option<ImageInput>) -> Result<Option<Image>, String> {
    let Some(input) = input else { return Ok(None) };
    let result = crate::media::call(&chats.data_dir, &json!({ "path": input.path, "title": input.title }));
    match crate::media::item(&result) {
        Some(Item::Media { title, asset }) if !asset.ends_with(".mp4") && !asset.ends_with(".webm") => Ok(Some(Image { title, asset })),
        _ => Err(result["content"][0]["text"].as_str().filter(|_| result["isError"] == true)
            .unwrap_or("Question previews must be PNG, JPEG, GIF, or WebP images.").to_owned()),
    }
}

fn prompts(chats: &Chats, input: Input) -> Result<Vec<Question>, String> {
    input.questions.into_iter().map(|question| {
        let options = question.options.into_iter().map(|option| Ok(ChoiceDescription {
            id: option.id, label: option.label, description: option.description, preview: option.preview,
            recommended: option.recommended, image: image(chats, option.image)?,
        })).collect::<Result<Vec<_>, String>>()?;
        Ok(Question {
            id: question.id, header: question.header, question: question.question,
            multi_select: question.answer_type == Type::Multiple, answer_type: Some(question.answer_type),
            image: image(chats, question.image)?, options,
        })
    }).collect()
}

pub fn valid_responses(questions: &[Question], responses: &HashMap<String, Response>) -> bool {
    responses.len() == questions.len() && questions.iter().all(|question| {
        let Some(response) = responses.get(&question.id) else { return false };
        if response.text.len() > 64 * 1024 { return false; }
        if response.skipped { return response.selected.is_empty() && response.text.is_empty(); }
        if response.selected.is_empty() && response.text.trim().is_empty() { return false; }
        let unique: HashSet<_> = response.selected.iter().collect();
        unique.len() == response.selected.len() && response.selected.iter().all(|id| question.options.iter().any(|option| option.id == *id))
            && match question.answer_type {
                Some(Type::Single) => response.selected.len() <= 1,
                Some(Type::Multiple) => true,
                Some(Type::Text) => response.selected.is_empty(),
                None => false,
            }
    })
}

fn error(message: impl ToString) -> Value {
    json!({ "content": [{ "type": "text", "text": message.to_string() }], "isError": true })
}

pub async fn ask(chats: &Chats, chat_id: Option<String>, arguments: Value, reader: &mut BufReader<OwnedReadHalf>) -> Value {
    let Some(chat_id) = chat_id else { return error("Questions require a calling Stampede chat.") };
    let Some(entry) = chats.get(&chat_id) else { return error("The calling chat is no longer open.") };
    let questions = match parse(arguments).and_then(|input| prompts(chats, input)) {
        Ok(questions) => questions,
        Err(cause) => return error(cause),
    };
    let id = format!("questions:{}", Uuid::new_v4());
    let (sender, receiver) = oneshot::channel();
    {
        let mut transcript = entry.transcript.lock().unwrap();
        if transcript.reported != Status::Working { return error("The calling agent is no longer working."); }
        transcript.questions.insert(id.clone(), sender);
        let request = Request { kind: RequestKind::Question { id: id.clone(), questions }, subagent: None };
        transcript.requests.push(request.clone());
        transcript.seq += 1;
        let _ = chats.changed.send(Change::Request { chat_id: chat_id.clone(), seq: transcript.seq, request });
        chats.settle(&entry, &transcript);
    }
    let mut disconnected = [0];
    let result = tokio::select! {
        response = receiver => match response {
            Ok(reply) => reply,
            Err(_) => Reply::cancelled(),
        },
        _ = reader.read(&mut disconnected) => Reply::cancelled(),
    };
    {
        let mut transcript = entry.transcript.lock().unwrap();
        if chats.resolve(&mut transcript, &chat_id, &id) { chats.settle(&entry, &transcript); }
    }
    json!({ "content": [{ "type": "text", "text": serde_json::to_string(&result).unwrap() }], "structuredContent": result, "isError": false })
}
