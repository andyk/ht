mod cli;
mod locale;
mod nbio;
use anyhow::{bail, Result};
use mio::unix::SourceFd;
use nix::libc;
use nix::pty;
use nix::sys::signal::{self, SigHandler, Signal};
use nix::sys::wait;
use nix::unistd::{self, ForkResult};
use std::env;
use std::ffi::{CString, NulError};
use std::io;
use std::os::fd::OwnedFd;
use std::sync::mpsc;
use std::thread;
use std::{
    fs::File,
    io::Write,
    os::fd::{AsRawFd, FromRawFd, RawFd},
};

#[derive(Debug)]
enum Message {
    Command(Command),
    Output(String),
    StdinClosed,
    ChildExited,
}

#[derive(Debug)]
enum Command {
    Input(String),
    GetView,
    Resize(usize, usize),
}

fn main() -> Result<()> {
    locale::check_utf8_locale()?;

    let cli = cli::Cli::new();
    let command = cli.command.join(" ");

    eprintln!(
        "launching command \"{command}\" in terminal of size {}",
        cli.size
    );

    let result = unsafe { pty::forkpty(Some(&*cli.size), None) }?;

    match result.fork_result {
        ForkResult::Parent { child } => {
            handle_parent(result.master.as_raw_fd(), child)?;
        }

        ForkResult::Child => {
            handle_child(command)?;
            unreachable!();
        }
    }

    Ok(())
}

fn handle_parent(master_fd: RawFd, child: unistd::Pid) -> Result<()> {
    let (sender, receiver) = mpsc::channel::<Message>();
    let (input_rx, input_tx) = nix::unistd::pipe()?;
    let input = unsafe { File::from_raw_fd(input_tx.as_raw_fd()) };
    let sender_ = sender.clone();

    thread::spawn(move || {
        let result = read_stdin(sender_.clone());
        let _ = sender_.send(Message::StdinClosed);

        result
    });

    let handle = thread::spawn(move || {
        let result = handle_process(master_fd, input_rx, sender.clone(), child);
        let _ = sender.send(Message::ChildExited);

        result
    });

    process_messages(receiver, input);

    handle.join().map_err(|e| anyhow::anyhow!("{e:?}"))?
}

fn handle_child<S>(command: S) -> io::Result<()>
where
    S: ToString,
{
    let command = ["/bin/sh".to_owned(), "-c".to_owned(), command.to_string()]
        .iter()
        .map(|s| CString::new(s.as_bytes()))
        .collect::<Result<Vec<CString>, NulError>>()?;

    env::set_var("TERM", "xterm-256color");
    unsafe { signal::signal(Signal::SIGPIPE, SigHandler::SigDfl) }?;
    unistd::execvp(&command[0], &command)?;
    unsafe { libc::_exit(1) }
}

fn read_stdin(sender: mpsc::Sender<Message>) -> Result<()> {
    for line in io::stdin().lines() {
        match serde_json::from_str::<serde_json::Value>(&line?) {
            Ok(json) => match json["type"].as_str() {
                Some("input") => {
                    let payload = json["payload"]
                        .as_str()
                        .ok_or(anyhow::anyhow!("payload missing"))?
                        .to_string();

                    sender.send(Message::Command(Command::Input(payload)))?;
                }

                Some("resize") => {
                    let cols = json["cols"]
                        .as_u64()
                        .ok_or(anyhow::anyhow!("cols missing"))?;

                    let rows = json["rows"]
                        .as_u64()
                        .ok_or(anyhow::anyhow!("rows missing"))?;

                    sender.send(Message::Command(Command::Resize(
                        cols as usize,
                        rows as usize,
                    )))?;
                }

                Some("getView") => {
                    sender.send(Message::Command(Command::GetView))?;
                }

                other => {
                    eprintln!("invalid command type: {other:?}");
                }
            },

            Err(e) => {
                eprintln!("JSON parse error: {e}");
            }
        }
    }

    Ok(())
}

fn handle_process(
    master_fd: RawFd,
    input_rx: OwnedFd,
    sender: mpsc::Sender<Message>,
    child: unistd::Pid,
) -> Result<()> {
    let result = handle_pty(master_fd, input_rx, sender.clone());
    eprintln!("killing the child with HUP");
    unsafe { libc::kill(child.as_raw(), libc::SIGHUP) };
    eprintln!("waiting for child exit");
    let _ = wait::waitpid(child, None);

    result
}

fn handle_pty(master_fd: RawFd, input_rx: OwnedFd, sender: mpsc::Sender<Message>) -> Result<()> {
    const MASTER: mio::Token = mio::Token(0);
    const INPUT: mio::Token = mio::Token(1);
    const BUF_SIZE: usize = 128 * 1024;

    let mut poll = mio::Poll::new()?;
    let mut events = mio::Events::with_capacity(128);
    let mut master_file = unsafe { File::from_raw_fd(master_fd) };
    let mut master_source = SourceFd(&master_fd);
    let input_fd = input_rx.as_raw_fd();
    let mut input_file = unsafe { File::from_raw_fd(input_fd) };
    let mut input_source = SourceFd(&input_fd);
    let mut buf = [0u8; BUF_SIZE];
    let mut input: Vec<u8> = Vec::with_capacity(BUF_SIZE);
    nbio::set_non_blocking(&input_fd)?;
    nbio::set_non_blocking(&master_fd)?;

    poll.registry()
        .register(&mut master_source, MASTER, mio::Interest::READABLE)?;

    poll.registry()
        .register(&mut input_source, INPUT, mio::Interest::READABLE)?;

    loop {
        if let Err(e) = poll.poll(&mut events, None) {
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            } else {
                bail!(e);
            }
        }

        for event in events.iter() {
            match event.token() {
                MASTER => {
                    if event.is_readable() {
                        while let Some(n) = nbio::read(&mut master_file, &mut buf)? {
                            if n > 0 {
                                sender.send(Message::Output(
                                    String::from_utf8_lossy(&buf[0..n]).to_string(),
                                ))?;
                            } else {
                                return Ok(());
                            }
                        }
                    }

                    if event.is_writable() {
                        let mut buf: &[u8] = input.as_ref();

                        while let Some(n) = nbio::write(&mut master_file, buf)? {
                            buf = &buf[n..];

                            if buf.is_empty() {
                                break;
                            }
                        }

                        let left = buf.len();

                        if left == 0 {
                            input.clear();

                            poll.registry().reregister(
                                &mut master_source,
                                MASTER,
                                mio::Interest::READABLE,
                            )?;
                        } else {
                            input.drain(..input.len() - left);
                        }
                    }

                    if event.is_read_closed() {
                        return Ok(());
                    }
                }

                INPUT => {
                    if event.is_readable() {
                        while let Some(n) = nbio::read(&mut input_file, &mut buf)? {
                            if n > 0 {
                                input.extend_from_slice(&buf[0..n]);

                                poll.registry().reregister(
                                    &mut master_source,
                                    MASTER,
                                    mio::Interest::READABLE | mio::Interest::WRITABLE,
                                )?;
                            } else {
                                return Ok(());
                            }
                        }
                    }

                    if event.is_read_closed() {
                        return Ok(());
                    }
                }

                _ => (),
            }
        }
    }
}

fn process_messages(receiver: mpsc::Receiver<Message>, mut input: File) {
    let mut vt = avt::Vt::builder().size(80, 24).resizable(true).build();

    for message in receiver {
        match message {
            Message::Command(Command::Input(i)) => {
                input.write_all(i.as_bytes()).unwrap();
            }

            Message::Command(Command::GetView) => {
                let text = vt
                    .lines()
                    .iter()
                    .map(|l| l.text())
                    .collect::<Vec<_>>()
                    .join("\n");

                let resp = serde_json::json!({ "view": text });
                println!("{}", serde_json::to_string(&resp).unwrap());
            }

            Message::Command(Command::Resize(cols, rows)) => {
                vt.feed_str(&format!("\x1b[8;{};{}t", rows, cols));
            }

            Message::Output(o) => {
                vt.feed_str(&o);
            }

            Message::StdinClosed => {
                eprintln!("stdin closed, closing child process input");
                std::mem::drop(input);
                break;
            }

            Message::ChildExited => {
                eprintln!("child process exited, closing stdin");
                let _ = nix::unistd::close(io::stdin().as_raw_fd());
                break;
            }
        }
    }
}
