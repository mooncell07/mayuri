# mayuri
An Asynchronous WebSocket Client Library.

# Usage
*This library uses callback-styled event dispatch*

```rs

use mayuri::{Context, WebSocket, bind};

/// Called when the WebSocket connection is established
#[bind(to = "on_connect")]
async fn handle_connect(ctx: Context) {
    println!("Connected to the server!");

    let message = "hello!";
    ctx.write_text(message.as_bytes()).await.unwrap();
    println!("Sent: {}", message);
}

/// Called when a message is received from the server
#[bind(to = "on_message")]
async fn handle_message(ctx: Context) {
    let received = ctx.read_text().unwrap();
    println!("Received: {}", received);

    let response = "mhmm";
    ctx.write_text(response.as_bytes()).await.unwrap();
    println!("Sent: {}", response);
}

#[tokio::main]
async fn main() {
    let server_url = "wss://ws.ifelse.io";
    let mut ws_client = WebSocket::connect(server_url).await.unwrap();

    ws_client.run().await.unwrap();
}

```
