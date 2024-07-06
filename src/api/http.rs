use super::Subscription;
use crate::session;
use anyhow::Result;
use axum::{
    extract::{connect_info::ConnectInfo, ws, Query, State},
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{sink, stream, StreamExt};
use rust_embed::RustEmbed;
use serde::Deserialize;
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
        .route("/ws/alis", get(alis_handler))
        .route("/ws/events", get(event_stream_handler))
        .with_state(clients_tx)
        .fallback(static_handler);

    Ok(axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .into_future())
}

/// ALiS protocol handler
///
/// This endpoint implements ALiS (asciinema live stream) protocol (https://docs.asciinema.org/manual/alis/).
/// It allows pointing asciinema player directly to ht to get a real-time terminal preview.
async fn alis_handler(
    ws: ws::WebSocketUpgrade,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    State(clients_tx): State<mpsc::Sender<session::Client>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let _ = handle_alis_socket(socket, clients_tx).await;
    })
}

async fn handle_alis_socket(
    socket: ws::WebSocket,
    clients_tx: mpsc::Sender<session::Client>,
) -> Result<()> {
    let (sink, stream) = socket.split();
    let drainer = tokio::spawn(stream.map(Ok).forward(sink::drain()));

    let result = session::stream(&clients_tx)
        .await?
        .filter_map(alis_message)
        .chain(stream::once(future::ready(Ok(close_message()))))
        .forward(sink)
        .await;

    drainer.abort();
    result?;

    Ok(())
}

async fn alis_message(
    event: Result<session::Event, BroadcastStreamRecvError>,
) -> Option<Result<ws::Message, axum::Error>> {
    use session::Event::*;

    match event {
        Ok(Init(time, cols, rows, seq, _text)) => Some(Ok(json_message(json!({
            "time": time,
            "cols": cols,
            "rows": rows,
            "init": seq,
        })))),

        Ok(Output(time, data)) => Some(Ok(json_message(json!([time, "o", data])))),

        Ok(Resize(time, cols, rows)) => Some(Ok(json_message(json!([
            time,
            "r",
            format!("{cols}x{rows}")
        ])))),

        Ok(Snapshot(_, _, _, _)) => None,

        Err(e) => Some(Err(axum::Error::new(e))),
    }
}

#[derive(Debug, Deserialize)]
struct EventsParams {
    sub: Option<String>,
}

/// Event stream handler
///
/// This endpoint allows the client to subscribe to selected events and have them delivered as they occur.
/// Query param `sub` should be set to a comma-separated list desired of events.
/// See above for a list of supported events.
async fn event_stream_handler(
    ws: ws::WebSocketUpgrade,
    Query(params): Query<EventsParams>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    State(clients_tx): State<mpsc::Sender<session::Client>>,
) -> impl IntoResponse {
    let sub: Subscription = params.sub.unwrap_or_default().parse().unwrap_or_default();

    ws.on_upgrade(move |socket| async move {
        let _ = handle_event_stream_socket(socket, clients_tx, sub).await;
    })
}

async fn handle_event_stream_socket(
    socket: ws::WebSocket,
    clients_tx: mpsc::Sender<session::Client>,
    sub: Subscription,
) -> Result<()> {
    let (sink, stream) = socket.split();
    let drainer = tokio::spawn(stream.map(Ok).forward(sink::drain()));

    let result = session::stream(&clients_tx)
        .await?
        .filter_map(move |e| event_stream_message(e, sub))
        .chain(stream::once(future::ready(Ok(close_message()))))
        .forward(sink)
        .await;

    drainer.abort();
    result?;

    Ok(())
}

async fn event_stream_message(
    event: Result<session::Event, BroadcastStreamRecvError>,
    sub: Subscription,
) -> Option<Result<ws::Message, axum::Error>> {
    use session::Event::*;

    match event {
        Ok(e @ Init(_, _, _, _, _)) if sub.init => Some(Ok(json_message(e.to_json()))),
        Ok(e @ Output(_, _)) if sub.output => Some(Ok(json_message(e.to_json()))),
        Ok(e @ Resize(_, _, _)) if sub.resize => Some(Ok(json_message(e.to_json()))),
        Ok(e @ Snapshot(_, _, _, _)) if sub.snapshot => Some(Ok(json_message(e.to_json()))),
        Ok(_) => None,
        Err(e) => Some(Err(axum::Error::new(e))),
    }
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
