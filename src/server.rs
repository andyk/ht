use crate::session;
use anyhow::Result;
use axum::{
    extract::{connect_info::ConnectInfo, ws, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{sink, stream, StreamExt};
use serde_json::json;
use std::borrow::Cow;
use std::future::{self, Future, IntoFuture};
use std::io;
use std::net::{SocketAddr, TcpListener};
use tokio::sync::mpsc;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub async fn start(
    listener: TcpListener,
    clients_tx: mpsc::Sender<session::Client>,
) -> Result<impl Future<Output = io::Result<()>>> {
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;

    eprintln!(
        "HTTP server listening on {}",
        listener.local_addr().unwrap()
    );

    let app: Router<()> = Router::new()
        .route("/ws/live", get(ws_handler))
        .with_state(clients_tx);

    Ok(axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .into_future())
}

async fn ws_handler(
    ws: ws::WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(clients_tx): State<mpsc::Sender<session::Client>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        eprintln!("websocket client {addr} connected");
        let _ = handle_socket(socket, clients_tx).await;
        eprintln!("websocket client {addr} disconnected");
    })
}

async fn handle_socket(
    socket: ws::WebSocket,
    clients_tx: mpsc::Sender<session::Client>,
) -> Result<()> {
    let (sink, stream) = socket.split();
    let drainer = tokio::spawn(stream.map(Ok).forward(sink::drain()));

    let result = session::stream(&clients_tx)
        .await?
        .map(ws_result)
        .chain(stream::once(future::ready(Ok(close_message()))))
        .forward(sink)
        .await;

    drainer.abort();
    result?;

    Ok(())
}

fn ws_result(
    event: Result<session::Event, BroadcastStreamRecvError>,
) -> Result<ws::Message, axum::Error> {
    use session::Event::*;

    event.map_err(axum::Error::new).map(|event| match event {
        Init(time, cols, rows, init) => json_message(json!({
            "cols": cols,
            "rows": rows,
            "time": time,
            "init": init
        })),

        Stdout(time, data) => json_message(json!([time, "o", data])),

        Resize(time, cols, rows) => json_message(json!([time, "r", format!("{cols}x{rows}")])),
    })
}

fn json_message(value: serde_json::Value) -> ws::Message {
    ws::Message::Text(value.to_string())
}

fn close_message() -> ws::Message {
    ws::Message::Close(Some(ws::CloseFrame {
        code: ws::close_code::NORMAL,
        reason: Cow::from("ended"),
    }))
}
