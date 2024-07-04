use crate::session;
use anyhow::Result;
use axum::{
    extract::{connect_info::ConnectInfo, ws, State},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{sink, stream, StreamExt};
use rust_embed::RustEmbed;
use serde_json::json;
use std::borrow::Cow;
use std::future::{self, Future, IntoFuture};
use std::io;
use std::net::{SocketAddr, TcpListener};
use tokio::sync::mpsc;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

#[derive(RustEmbed)]
#[folder = "assets/"]
struct Assets;

pub async fn start(
    listener: TcpListener,
    clients_tx: mpsc::Sender<session::Client>,
) -> Result<impl Future<Output = io::Result<()>>> {
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let addr = listener.local_addr().unwrap();
    eprintln!("HTTP server listening on {addr}");
    eprintln!("live preview available at http://{addr}");

    let app: Router<()> = Router::new()
        .route("/ws/live", get(ws_handler))
        .with_state(clients_tx)
        .fallback(static_handler);

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

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/');

    if path.is_empty() {
        path = "index.html";
    }

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();

            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }

        None => (StatusCode::NOT_FOUND, "404").into_response(),
    }
}
