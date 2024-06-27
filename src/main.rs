mod api;
mod cli;
mod command;
mod locale;
mod nbio;
mod pty;
mod server;
mod vt;

use anyhow::Result;
use command::Command;
use tokio::{sync::mpsc, task::JoinHandle};
use vt::Vt;

#[tokio::main]
async fn main() -> Result<()> {
    locale::check_utf8_locale()?;

    let cli = cli::Cli::new();
    let command = cli.command.join(" ");

    eprintln!(
        "launching command \"{command}\" in terminal of size {}",
        cli.size
    );

    let vt = Vt::new(cli.size.cols(), cli.size.rows());
    let (input_tx, input_rx) = mpsc::channel(1024);
    let (output_tx, output_rx) = mpsc::channel(1024);
    let (command_tx, command_rx) = mpsc::channel(1024);
    let pty_handle = tokio::spawn(pty::spawn(command, &cli.size, input_rx, output_tx)?);
    let api_handle = tokio::spawn(api::start(command_tx));
    let _server_handle = tokio::spawn(server::start().await?);
    event_loop(output_rx, input_tx, command_rx, vt, api_handle).await?;
    pty_handle.await??;

    Ok(())
}

async fn event_loop(
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
