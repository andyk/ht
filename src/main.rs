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
use tokio::sync::mpsc;
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

    let (input_tx, input_rx) = mpsc::channel(1024);
    let (output_tx, output_rx) = mpsc::channel(1024);
    let handle = tokio::spawn(pty::spawn(command, &cli.size, input_rx, output_tx)?);
    let vt = Vt::new(cli.size.cols(), cli.size.rows());
    event_loop(output_rx, input_tx, vt).await?;
    handle.await??;

    Ok(())
}

async fn event_loop(
    mut output_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
    input_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    mut vt: Vt,
) -> Result<()> {
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1024);
    let mut api = tokio::spawn(api::start(command_tx));
    let _server = tokio::spawn(server::start());

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

            _ = &mut api => {
                eprintln!("stdin closed, shutting down...");
                break;
            }
        }
    }

    Ok(())
}
