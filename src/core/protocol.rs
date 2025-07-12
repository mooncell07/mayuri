use super::{context::Context, transport::Transport};
use async_trait::async_trait;

#[async_trait]
pub trait WebSocketProtocol {
    async fn on_connect(&mut self, transport: Transport);
    async fn on_message(&mut self, ctx: Context);
    async fn on_close(&mut self, ctx: Context);
}
