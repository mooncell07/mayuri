use super::{enums::Event, frame::Frame};

pub struct Context {
    pub belongs_to: Event,
    frame: Frame,
}

impl Context {
    pub fn new(event: Event, frame: Frame) -> Context {
        return Self {
            belongs_to: event,
            frame,
        };
    }

    pub fn read_text(&self) -> String {
        String::from_utf8_lossy(&self.frame.payload_data).to_string()
    }
}
