use std::{
    io::{self, Read},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::hooks::Status;

pub const VERSION: u16 = 5;
pub const MAX_FRAME: usize = 256 << 20;

const HELLO: u8 = 1;
const ATTACH: u8 = 2;
const SNAPSHOT: u8 = 3;
const OUTPUT: u8 = 4;
const INPUT: u8 = 5;
const RESIZE: u8 = 6;
const HANGUP: u8 = 7;
const FOREGROUND_CHANGED: u8 = 8;
const AGENT: u8 = 9;
const TITLE_CHANGED: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u16,
    pub holder_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Foreground {
    pub is_root_shell: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentState {
    pub status: Option<Status>,
    pub session: Option<String>,
    pub viewed_since_done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Title {
    pub title: Option<String>,
    pub cwd: PathBuf,
}

#[derive(Debug)]
pub enum Message {
    Hello(Hello),
    Attach,
    Snapshot(Vec<u8>),
    Output(Vec<u8>),
    Input(Vec<u8>),
    Resize(Size),
    Hangup,
    ForegroundChanged(Foreground),
    Agent(AgentState),
    TitleChanged(Title),
}

pub fn encode(message: &Message) -> Vec<u8> {
    let (tag, payload) = match message {
        Message::Hello(hello) => (HELLO, serde_json::to_vec(hello).unwrap()),
        Message::Attach => (ATTACH, Vec::new()),
        Message::Snapshot(bytes) => (SNAPSHOT, bytes.clone()),
        Message::Output(bytes) => (OUTPUT, bytes.clone()),
        Message::Input(bytes) => (INPUT, bytes.clone()),
        Message::Resize(size) => (RESIZE, serde_json::to_vec(size).unwrap()),
        Message::Hangup => (HANGUP, Vec::new()),
        Message::ForegroundChanged(foreground) => {
            (FOREGROUND_CHANGED, serde_json::to_vec(foreground).unwrap())
        }
        Message::Agent(state) => (AGENT, serde_json::to_vec(state).unwrap()),
        Message::TitleChanged(title) => (TITLE_CHANGED, serde_json::to_vec(title).unwrap()),
    };
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32 + 1).to_be_bytes());
    frame.push(tag);
    frame.extend_from_slice(&payload);
    frame
}

/// Decodes a frame without its length prefix. Unknown tags decode to `None`,
/// so a version 1 peer skips kinds that later versions add.
pub fn decode(frame: &[u8]) -> io::Result<Option<Message>> {
    let Some((&tag, payload)) = frame.split_first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty frame"));
    };
    Ok(Some(match tag {
        HELLO => Message::Hello(serde_json::from_slice(payload)?),
        ATTACH => Message::Attach,
        SNAPSHOT => Message::Snapshot(payload.to_vec()),
        OUTPUT => Message::Output(payload.to_vec()),
        INPUT => Message::Input(payload.to_vec()),
        RESIZE => Message::Resize(serde_json::from_slice(payload)?),
        HANGUP => Message::Hangup,
        FOREGROUND_CHANGED => Message::ForegroundChanged(serde_json::from_slice(payload)?),
        AGENT => Message::Agent(serde_json::from_slice(payload)?),
        TITLE_CHANGED => Message::TitleChanged(serde_json::from_slice(payload)?),
        _ => return Ok(None),
    }))
}

pub fn read(reader: &mut impl Read) -> io::Result<Option<Message>> {
    loop {
        let mut len = [0; 4];
        match reader.read_exact(&mut len) {
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            result => result?,
        }
        let len = u32::from_be_bytes(len) as usize;
        if len > MAX_FRAME {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
        }
        let mut frame = vec![0; len];
        reader.read_exact(&mut frame)?;
        if let Some(message) = decode(&frame)? {
            return Ok(Some(message));
        }
    }
}
