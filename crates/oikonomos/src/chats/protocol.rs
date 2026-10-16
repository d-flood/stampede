use std::io::{self, Read};

use serde::{Deserialize, Serialize};

use crate::protocol::MAX_FRAME;

pub const VERSION: u16 = 2;

const HELLO: u8 = 1;
const LINE: u8 = 2;
const INPUT: u8 = 3;
const HANGUP: u8 = 4;
const FORGET: u8 = 5;

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u16,
    pub holder_version: String,
    /// How many buffered `Line` and `Input` frames follow this one.
    #[serde(default)]
    pub buffered: usize,
}

#[derive(Debug)]
pub enum Message {
    Hello(Hello),
    Line(Vec<u8>),
    Input(Vec<u8>),
    Hangup,
    Forget,
}

pub fn encode(message: &Message) -> Vec<u8> {
    let (tag, payload) = match message {
        Message::Hello(hello) => (HELLO, serde_json::to_vec(hello).unwrap()),
        Message::Line(bytes) => (LINE, bytes.clone()),
        Message::Input(bytes) => (INPUT, bytes.clone()),
        Message::Hangup => (HANGUP, Vec::new()),
        Message::Forget => (FORGET, Vec::new()),
    };
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32 + 1).to_be_bytes());
    frame.push(tag);
    frame.extend_from_slice(&payload);
    frame
}

/// Decodes a frame without its length prefix. Unknown tags decode to `None`.
pub fn decode(frame: &[u8]) -> io::Result<Option<Message>> {
    let Some((&tag, payload)) = frame.split_first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty frame"));
    };
    Ok(Some(match tag {
        HELLO => Message::Hello(serde_json::from_slice(payload)?),
        LINE => Message::Line(payload.to_vec()),
        INPUT => Message::Input(payload.to_vec()),
        HANGUP => Message::Hangup,
        FORGET => Message::Forget,
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
