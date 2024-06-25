pub async fn start() {
    eprintln!("HTTP server started");
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    eprintln!("HTTP server exited");
}
