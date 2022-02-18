#[macro_use]
extern crate log;

use futures_util::{StreamExt, SinkExt};
use warp::{Filter, ws::{WebSocket, Message}};

#[tokio::main]
async fn main() {
    env_logger::init();
    let routes = warp::path("ws")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            // And then our closure will be called when it completes...
            ws.on_upgrade(|ws| client_connection(ws))
        });

    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
}

pub async fn client_connection(ws: WebSocket) {
    println!("establishing client connection... {:?}", ws);
    let (mut client_ws_sender, _client_ws_rcv) = ws.split();
    // let (tx, rx) = mpsc::unbounded_channel();
    let nc = async_nats::connect("nats:4222").await.unwrap();
    let sub = nc.subscribe("video.1").await.unwrap();
    loop {
        match sub.next().await {
            Some(m) => {
                info!("Forwarding video message");
                client_ws_sender.send(Message::binary(m.data)).await.unwrap();
            },
            None => {}
        };
    };
}
