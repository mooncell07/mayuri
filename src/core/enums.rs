use std::io;
use strum::FromRepr;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, FromRepr)]
pub enum State {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
    ERROR = 4,
}

impl State {
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    pub fn from_u8(opcode: u8) -> Result<Self, io::Error> {
        Ok(match opcode {
            0x0 => Self::Continuation,
            0x1 => Self::Text,
            0x2 => Self::Binary,
            0x8 => Self::Close,
            0x9 => Self::Ping,
            0xA => Self::Pong,
            _ => {
                return Err(io::Error::other(format!("Bad Opcode {opcode}")));
            }
        })
    }
}
