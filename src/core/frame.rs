use super::{
    enums::Opcode,
    errors::{HandshakeFailureError, WebSocketError},
    utils::CRLF,
};
use crate::safe_get_handshake_item;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use rand::RngCore;
use std::{
    collections::HashMap,
    io::{Cursor, Read, Write},
};
use tokio::io;
const EXPECTED_STATUS_LINE: &str = "HTTP/1.1 101 Switching Protocols";
const MIN_VAL_FOR_16_BIT_UPGRADE: u8 = 0x7E;
const MIN_VAL_FOR_64_BIT_UPGRADE: u8 = 0x7F;

#[derive(Debug)]
pub struct HandshakeHeaders {
    pub http_version: String,
    pub http_status_code: String,
    pub http_status_text: String,
    pub headers: HashMap<String, String>,
}

impl HandshakeHeaders {
    pub fn new(data: &str) -> Result<Self, WebSocketError> {
        let lines: Vec<&str> = data.split(CRLF).collect();

        let first_line = safe_get_handshake_item!(lines, 0, "status line")?;

        if *first_line != EXPECTED_STATUS_LINE {
            return Err(WebSocketError::Handshake(
                HandshakeFailureError::HeaderError(format!(
                    "Bad Status Line in handshake response: {first_line}"
                )),
            ));
        }

        let headers_meta: Vec<&str> = first_line.split_whitespace().collect();
        let headers: HashMap<String, String> = lines
            .iter()
            .filter_map(|line| {
                if let Some((key, value)) = line.split_once(':') {
                    Some((key.trim().to_lowercase(), value.trim().to_string()))
                } else {
                    None
                }
            })
            .collect();

        let http_version =
            (*safe_get_handshake_item!(headers_meta, 0, "http version")?).to_string();
        let http_status_code =
            (*safe_get_handshake_item!(headers_meta, 1, "http status code")?).to_string();

        let http_status_text =
            (*safe_get_handshake_item!(headers_meta, 2.., "http status text")?).join(" ");

        Ok(Self {
            http_version,
            http_status_code,
            http_status_text,
            headers,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Headers {
    pub fin: bool,
    pub rsv1: bool,
    pub rsv2: bool,
    pub rsv3: bool,
    pub opcode: Opcode,
    pub mask: bool,
    pub payload_len: u8,
    pub payload_len_ext: u64,

    // `extend_by` is 16 when `payload_len` is 126, this makes `stream` read a 16 bit
    // uint `payload_len_ext` value. 64 when `payload_len` is 127.
    // Only used during frame decoding. Set to 0 when encoding or when `payload_len` is
    // enough.
    pub extend_by: u8,
}

impl Headers {
    pub fn decode(data: &[u8]) -> Result<Self, io::Error> {
        let mut cursor = Cursor::new(data);
        let byte0 = cursor.read_u8()?;
        let byte1 = cursor.read_u8()?;

        let fin = (byte0 >> 7) != 0;
        let rsv1 = (byte0 >> 6) != 0;
        let rsv2 = (byte0 >> 5) != 0;
        let rsv3 = (byte0 >> 4) != 0;
        let opcode = Opcode::from_u8(byte0 & 0x0F)?;

        let mask = (byte1 >> 7) != 0;
        let payload_len = byte1 & 0x7F;

        let extend_by = if payload_len == MIN_VAL_FOR_16_BIT_UPGRADE {
            16
        } else if payload_len == MIN_VAL_FOR_64_BIT_UPGRADE {
            64
        } else {
            0
        };

        Ok(Self {
            fin,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            mask,
            payload_len,
            payload_len_ext: 0,
            extend_by,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, io::Error> {
        let mut cursor = Cursor::new(Vec::new());
        let byte0 = (u8::from(self.fin) << 7)
            | (u8::from(self.rsv1) << 6)
            | (u8::from(self.rsv2) << 5)
            | (u8::from(self.rsv3) << 4)
            | self.opcode as u8;
        cursor.write_u8(byte0)?;

        let byte1 = (u8::from(self.mask) << 7) | self.payload_len;
        cursor.write_u8(byte1)?;

        if self.payload_len == MIN_VAL_FOR_16_BIT_UPGRADE {
            cursor.write_u16::<BigEndian>(self.payload_len_ext as u16)?;
        } else if self.payload_len == MIN_VAL_FOR_64_BIT_UPGRADE {
            cursor.write_u64::<BigEndian>(self.payload_len_ext)?;
        }

        Ok(cursor.into_inner())
    }

    #[must_use]
    pub const fn set_defaults(opcode: Opcode, payload_len: u8, payload_len_ext: u64) -> Self {
        Self {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode,
            mask: true,
            payload_len,
            payload_len_ext,
            extend_by: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub headers: Headers,
    pub payload_data: Vec<u8>,
}

impl Frame {
    pub fn decode(data: &[u8], headers: Headers) -> Result<Self, io::Error> {
        let mut cursor = Cursor::new(data);

        let final_payload_len = if headers.extend_by > 0 {
            headers.payload_len_ext
        } else {
            u64::from(headers.payload_len)
        };

        let mut payload_data = vec![0u8; final_payload_len as usize];
        cursor.read_exact(&mut payload_data)?;

        Ok(Self {
            headers,
            payload_data,
        })
    }

    pub fn encode(&mut self) -> Result<Vec<u8>, io::Error> {
        let mut cursor = Cursor::new(Vec::new());
        let masking_key = Self::get_masking_key();

        for (i, byte) in self.payload_data.iter_mut().enumerate() {
            let key = 3 - (i % 4);
            *byte ^= ((masking_key >> (8 * key)) & 0xFF) as u8;
        }

        cursor.write_all(&self.headers.encode()?)?;
        cursor.write_u32::<BigEndian>(masking_key)?;
        cursor.write_all(&self.payload_data)?;
        Ok(cursor.into_inner())
    }

    #[must_use]
    pub fn set_defaults(opcode: Opcode, data: &[u8]) -> Self {
        let (payload_len, payload_len_ext) = Self::get_payload_len(data.len());
        let headers = Headers::set_defaults(opcode, payload_len, payload_len_ext);
        Self {
            headers,
            payload_data: data.to_vec(),
        }
    }

    const fn get_payload_len(len: usize) -> (u8, u64) {
        if len < MIN_VAL_FOR_16_BIT_UPGRADE as usize {
            (len as u8, 0u64)
        } else if len <= 0xFFFF {
            (MIN_VAL_FOR_16_BIT_UPGRADE, len as u64)
        } else {
            (MIN_VAL_FOR_64_BIT_UPGRADE, len as u64)
        }
    }

    fn get_masking_key() -> u32 {
        let mut rng = rand::rng();
        rng.next_u32()
    }
}
