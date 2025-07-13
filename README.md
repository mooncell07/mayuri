# mayuri
A WIP Asynchronous WebSocket Client Library.

## Installation
Install the [mayuri](https://crates.io/crates/mayuri) crate from crates-io.

## Usage

```rs
use async_trait::async_trait;
use mayuri::{Context, Transport, WebSocket, WebSocketProtocol};
use tokio;

pub struct App {}

#[async_trait]
impl WebSocketProtocol for App {
    async fn on_connect(&mut self, mut transport: Transport) {
        println!("Connected!");
        transport.write_text(b"hello!").await.unwrap();
    }

    async fn on_message(&mut self, ctx: Context) {
        let msg = ctx.read_text();
        println!("Received: {}", msg);
    }

    async fn on_close(&mut self, _: Context) {
        println!("Closed by the Peer!");
    }
}

#[tokio::main]
async fn main() {
    let mut ws = WebSocket::connect("wss://ws.ifelse.io", App {})
        .await
        .unwrap();
    ws.run().await.unwrap();
}
```
Find more in [examples](../master/examples/).
