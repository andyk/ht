mod api;
mod cli;
mod command;
mod locale;
mod nbio;
mod pty;
mod session;
use anyhow::{Context, Result};
use command::{Command, InputSeq};
use session::Session;
use std::net::{SocketAddr, TcpListener};
use tokio::{sync::mpsc, task::JoinHandle};

#[tokio::main]
async fn main() -> Result<()> {
    locale::check_utf8_locale()?;
    let cli = cli::Cli::new();

    let (input_tx, input_rx) = mpsc::channel(1024);
    let (output_tx, output_rx) = mpsc::channel(1024);
    let (command_tx, command_rx) = mpsc::channel(1024);
    let (clients_tx, clients_rx) = mpsc::channel(1);

    start_http_api(cli.listen, clients_tx.clone()).await?;
    let api = start_stdio_api(command_tx, clients_tx, cli.subscribe.unwrap_or_default());
    let pty = start_pty(cli.command, &cli.size, input_rx, output_tx)?;
    let session = build_session(&cli.size);
    run_event_loop(output_rx, input_tx, command_rx, clients_rx, session, api).await?;
    pty.await?
}

fn build_session(size: &cli::Size) -> Session {
    Session::new(size.cols(), size.rows())
}

fn start_stdio_api(
    command_tx: mpsc::Sender<Command>,
    clients_tx: mpsc::Sender<session::Client>,
    sub: api::Subscription,
) -> JoinHandle<Result<()>> {
    tokio::spawn(api::stdio::start(command_tx, clients_tx, sub))
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

async fn start_http_api(
    listen_addr: Option<SocketAddr>,
    clients_tx: mpsc::Sender<session::Client>,
) -> Result<()> {
    if let Some(addr) = listen_addr {
        let listener = TcpListener::bind(addr).context("cannot start HTTP listener")?;
        tokio::spawn(api::http::start(listener, clients_tx).await?);
    }

    Ok(())
}

async fn run_event_loop(
    mut output_rx: mpsc::Receiver<Vec<u8>>,
    input_tx: mpsc::Sender<Vec<u8>>,
    mut command_rx: mpsc::Receiver<Command>,
    mut clients_rx: mpsc::Receiver<session::Client>,
    mut session: Session,
    mut api_handle: JoinHandle<Result<()>>,
) -> Result<()> {
    let cli = cli::Cli::new();
    let mut serving = true;
    let mut process_exited = false;
    let mut wait_for_enter = false;

    loop {
        tokio::select! {
            result = output_rx.recv() => {
                match result {
                    Some(data) => {
                        session.output(String::from_utf8_lossy(&data).to_string());
                    },

                    None => {
                        if !process_exited && cli.no_exit {
                            eprintln!("Process exited. Send {{\"type\": \"sendKeys\", \"keys\": [\"Enter\"]}} to exit...");
                            process_exited = true;
                            wait_for_enter = true;
                        } else if !cli.no_exit {
                            eprintln!("Process exited, shutting down...");
                            break;
                        }
                    }
                }
            }

            command = command_rx.recv() => {
                match command {
                    Some(Command::Input(seqs)) => {
                        if wait_for_enter {
                            // Check if Enter was pressed
                            for seq in &seqs {
                                if let InputSeq::Standard(key) = seq {
                                    if key == "\r" || key == "\n" {
                                        eprintln!("Enter command received, shutting down...");
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        
                        let data = command::seqs_to_bytes(&seqs, session.cursor_key_app_mode());
                        input_tx.send(data).await?;
                    }

                    Some(Command::Snapshot) => {
                        session.snapshot();
                    }

                    Some(Command::Resize(cols, rows)) => {
                        session.resize(cols, rows);
                    }

                    None => {
                        if !process_exited {
                            eprintln!("stdin closed, shutting down...");
                            break;
                        }
                    }
                }
            }

            client = clients_rx.recv(), if serving => {
                match client {
                    Some(client) => {
                        client.accept(session.subscribe());
                    }

                    None => {
                        serving = false;
                    }
                }
            }

            _ = &mut api_handle => {
                if !process_exited {
                    eprintln!("API handle closed, shutting down...");
                    break;
                }
            }
        }
    }

    Ok(())
}
