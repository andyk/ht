use crate::command;
use anyhow::Result;
use std::io;
use std::thread;
use tokio::sync::mpsc;

pub async fn start(command_tx: mpsc::Sender<command::Command>) -> Result<()> {
    let (input_tx, mut input_rx) = mpsc::unbounded_channel();
    thread::spawn(|| read_stdin(input_tx));

    while let Some(line) = input_rx.recv().await {
        match command::parse(&line) {
            Ok(command) => command_tx.send(command).await?,
            Err(e) => eprintln!("command parse error: {e}"),
        }
    }

    Ok(())
}

fn read_stdin(input_tx: mpsc::UnboundedSender<String>) -> Result<()> {
    for line in io::stdin().lines() {
        input_tx.send(line?)?;
    }

    Ok(())
}
