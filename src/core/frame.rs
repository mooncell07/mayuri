use super::enums::Opcode;
use super::errors::HandshakeFailureError;
use super::utils::CRLF;
use byteorder::BigEndian;
use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;
use rand::RngCore;
use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;

const EXPECTED_STATUS_TEXT: &str = "HTTP/1.1 101 Switching Protocols";
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
    pub fn new(data: &str) -> Result<HandshakeHeaders, HandshakeFailureError> {
        let lines: Vec<&str> = data.split(CRLF).collect();

        if lines[0] != EXPECTED_STATUS_TEXT {
            return Err(HandshakeFailureError::HeaderError);
        }

        let headers_meta: Vec<&str> = lines[0].split_whitespace().collect();
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

        Ok(Self {
            http_version: headers_meta[0].to_string(),
            http_status_code: headers_meta[1].to_string(),
            http_status_text: headers_meta[2..].join(" "),
            headers,
        })
    }
}

#[derive(Debug)]
pub struct Frame {
    pub fin: bool,
    pub rsv1: bool,
    pub rsv2: bool,
    pub rsv3: bool,
    pub opcode: Opcode,
    pub mask: bool,
    pub payload_len: u8,
    pub payload_len_ext: u64,
    pub masking_key: u32,
    pub payload_data: Vec<u8>,
}

impl Frame {
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
        let final_payload_len: u64 = if payload_len == MIN_VAL_FOR_16_BIT_UPGRADE {
            cursor.read_u16::<BigEndian>()? as u64
        } else if payload_len == MIN_VAL_FOR_64_BIT_UPGRADE {
            cursor.read_u64::<BigEndian>()?
        } else {
            payload_len as u64
        };

        let payload_len_ext = if payload_len >= MIN_VAL_FOR_16_BIT_UPGRADE {
            final_payload_len
        } else {
            0u64
        };
        let mut payload_data = vec![0u8; final_payload_len as usize];

        cursor.read_exact(&mut payload_data)?;

        Ok(Self {
            fin,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            mask,
            payload_len,
            payload_len_ext,
            masking_key: 0,
            payload_data,
        })
    }

    pub fn encode(&mut self) -> Result<Vec<u8>, io::Error> {
        let mut cursor = Cursor::new(Vec::new());

        let byte0 = ((self.fin as u8) << 7)
            | ((self.rsv1 as u8) << 6)
            | ((self.rsv2 as u8) << 5)
            | ((self.rsv3 as u8) << 4)
            | self.opcode as u8;
        cursor.write_u8(byte0)?;

        let byte1 = ((self.mask as u8) << 7) | self.payload_len;
        cursor.write_u8(byte1)?;

        if self.payload_len == MIN_VAL_FOR_16_BIT_UPGRADE {
            cursor.write_u16::<BigEndian>(self.payload_len_ext as u16)?;
        } else if self.payload_len == MIN_VAL_FOR_64_BIT_UPGRADE {
            cursor.write_u64::<BigEndian>(self.payload_len_ext)?;
        }

        cursor.write_u32::<BigEndian>(self.masking_key)?;

        for (i, byte) in self.payload_data.iter_mut().enumerate() {
            let key = 3 - (i % 4);
            *byte ^= ((self.masking_key >> (8 * key)) & 0xFF) as u8;
        }

        cursor.write_all(&self.payload_data)?;
        Ok(cursor.into_inner())
    }

    pub fn set_defaults(opcode: Opcode, data: &[u8]) -> Self {
        let (payload_len, payload_len_ext) = Self::_set_payload_len(data.len());
        let masking_key = Self::_get_masking_key();

        Self {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode,
            mask: true,
            payload_len,
            payload_len_ext,
            masking_key,
            payload_data: data.to_vec(),
        }
    }

    pub fn _set_payload_len(len: usize) -> (u8, u64) {
        if len < MIN_VAL_FOR_16_BIT_UPGRADE as usize {
            (len as u8, 0u64)
        } else if len <= 0xFFFF {
            (MIN_VAL_FOR_16_BIT_UPGRADE, len as u64)
        } else {
            (MIN_VAL_FOR_64_BIT_UPGRADE, len as u64)
        }
    }

    pub fn _get_masking_key() -> u32 {
        let mut rng = rand::rng();
        rng.next_u32()
    }
}
