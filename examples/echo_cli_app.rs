use async_trait::async_trait;
use mayuri::{Context, Transport, WebSocket, WebSocketProtocol};
use tokio;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct App {
    _transport: OnceCell<Transport>,
}

impl App {
    fn get_transport(&mut self) -> &mut Transport {
        self._transport.get_mut().unwrap()
    }

    async fn send(&mut self, msg: &[u8]) {
        self.get_transport().write_text(msg).await.unwrap();
    }

    async fn run(&mut self) {
        let input = BufReader::new(stdin());
        let mut lines = input.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let content = line.trim();
            self.send(content.as_bytes()).await;
            println!("Sent: {}", content);
        }
    }
}

#[async_trait]
impl WebSocketProtocol for App {
    async fn on_connect(&mut self, transport: Transport) {
        self._transport.set(transport).unwrap();
        let mut app = self.clone();
        tokio::spawn(async move { app.run().await });
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
    let uri = "wss://ws.ifelse.io";
    let app = App {
        _transport: OnceCell::new(),
    };
    let mut ws = WebSocket::connect(uri, app).await.unwrap();
    ws.run().await.unwrap();
}
