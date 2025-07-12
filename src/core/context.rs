use std::io;

use super::frame::Frame;

pub struct Context {
    pub frame: Frame,
}

impl Context {
    pub const fn new(frame: Frame) -> Result<Self, io::Error> {
        Ok(Self { frame })
    }

    #[must_use]
    pub fn read_text(&self) -> String {
        String::from_utf8_lossy(&self.frame.payload_data).to_string()
    }
}
