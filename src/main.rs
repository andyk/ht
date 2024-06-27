mod api;
mod cli;
mod command;
mod locale;
mod nbio;
mod pty;
mod server;
mod vt;
use anyhow::{Context, Result};
use command::Command;
use std::net::{SocketAddr, TcpListener};
use tokio::{sync::mpsc, task::JoinHandle};
use vt::Vt;

#[tokio::main]
async fn main() -> Result<()> {
    locale::check_utf8_locale()?;
    let cli = cli::Cli::new();

    let (input_tx, input_rx) = mpsc::channel(1024);
    let (output_tx, output_rx) = mpsc::channel(1024);
    let (command_tx, command_rx) = mpsc::channel(1024);

    start_http_server(cli.listen_addr).await?;
    let api = start_api(command_tx);
    let pty = start_pty(cli.command, &cli.size, input_rx, output_tx)?;
    let vt = build_vt(&cli.size);
    run_event_loop(output_rx, input_tx, command_rx, vt, api).await?;
    pty.await?
}

fn build_vt(size: &cli::Size) -> vt::Vt {
    Vt::new(size.cols(), size.rows())
}

fn start_api(command_tx: mpsc::Sender<Command>) -> JoinHandle<Result<()>> {
    tokio::spawn(api::start(command_tx))
}

fn start_pty(
    command: Vec<String>,
    size: &cli::Size,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
) -> Result<JoinHandle<Result<()>>> {
    let command = command.join(" ");
    eprintln!("launching \"{}\" in terminal of size {}", command, size);

    Ok(tokio::spawn(pty::spawn(
        command, size, input_rx, output_tx,
    )?))
}

async fn start_http_server(listen_addr: Option<SocketAddr>) -> Result<()> {
    if let Some(addr) = listen_addr {
        let listener = TcpListener::bind(addr).context("cannot start HTTP listener")?;
        let _ = tokio::spawn(server::start(listener).await?);
    }

    Ok(())
}

async fn run_event_loop(
    mut output_rx: mpsc::Receiver<Vec<u8>>,
    input_tx: mpsc::Sender<Vec<u8>>,
    mut command_rx: mpsc::Receiver<Command>,
    mut vt: Vt,
    mut api_handle: JoinHandle<Result<()>>,
) -> Result<()> {
    loop {
        tokio::select! {
            result = output_rx.recv() => {
                match result {
                    Some(data) => { vt.feed_bytes(&data); },

                    None => {
                        eprintln!("process exited, shutting down...");
                        break;
                    }
                }
            }

            command = command_rx.recv() => {
                match command {
                    Some(Command::Input(seqs)) => {
                        let data = command::seqs_to_bytes(&seqs, vt.cursor_key_app_mode());
                        input_tx.send(data).await?;
                    }

                    Some(Command::GetView) => {
                        let resp = serde_json::json!({ "view": vt.get_text() });
                        println!("{}", serde_json::to_string(&resp).unwrap());
                    }

                    Some(Command::Resize(cols, rows)) => {
                        vt.resize(cols, rows);
                    }

                    None => {
                        eprintln!("stdin closed, shutting down...");
                        break;
                    }
                }
            }

            _ = &mut api_handle => {
                eprintln!("stdin closed, shutting down...");
                break;
            }
        }
    }

    Ok(())
}
