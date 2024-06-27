use anyhow::Result;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::IntoResponse;
use axum::Router;
use std::future::Future;
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::net::TcpListener;

pub async fn start(
    listener: TcpListener,
) -> Result<impl Future<Output = std::result::Result<(), std::io::Error>>> {
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;

    eprintln!(
        "HTTP server listening on {}",
        listener.local_addr().unwrap()
    );

    let app: Router<()> = Router::new().fallback(hello_world);

    Ok(axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .into_future())
}

async fn hello_world(_uri: Uri) -> impl IntoResponse {
    (StatusCode::OK, "Hello world!").into_response()
}
