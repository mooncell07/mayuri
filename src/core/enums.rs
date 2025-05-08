use std::io;

#[derive(Debug)]
pub enum Event {
    OnCONNECT,
    OnMESSAGE,
    OnERROR,
    OnDISCONNECT,
}

#[derive(Debug)]
pub enum State {
    CONNECTING = 0,
    OPEN = 1,
    CLOSING = 2,
    CLOSED = 3,
}

#[derive(Copy, Clone, Debug)]
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
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Bad Opcode {}", opcode),
                ));
            }
        })
    }
}
