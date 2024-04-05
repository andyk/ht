mod nbio;
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
}

fn main() {
    let winsize = pty::Winsize {
        ws_col: 80,
        ws_row: 24,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let result = unsafe { pty::forkpty(Some(&winsize), None) }.unwrap();

    match result.fork_result {
        ForkResult::Parent { child } => {
            handle_parent(result.master.as_raw_fd(), child);
        }

        ForkResult::Child => {
            handle_child("bash").unwrap();
            unreachable!();
        }
    }
}

fn handle_parent(master_fd: RawFd, child: unistd::Pid) {
    let (sender, receiver) = mpsc::channel::<Message>();
    let (input_rx, input_tx) = nix::unistd::pipe().unwrap();
    let input = unsafe { File::from_raw_fd(input_tx.as_raw_fd()) };
    let sender_ = sender.clone();

    thread::scope(|s| {
        s.spawn(move || read_stdin(sender_));
        s.spawn(move || handle_process(master_fd, input_rx, sender, child));
        process_messages(receiver, input);
    });
}

fn handle_child<S>(command: S) -> io::Result<()>
where
    S: ToString,
{
    let command = vec!["/bin/sh".to_owned(), "-c".to_owned(), command.to_string()]
        .iter()
        .map(|s| CString::new(s.as_bytes()))
        .collect::<Result<Vec<CString>, NulError>>()
        .unwrap();

    env::set_var("TERM", "xterm-256color");
    unsafe { signal::signal(Signal::SIGPIPE, SigHandler::SigDfl) }.unwrap();
    unistd::execvp(&command[0], &command).unwrap();
    unsafe { libc::_exit(1) }
}

fn read_stdin(sender: mpsc::Sender<Message>) {
    for line in io::stdin().lines() {
        let json: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();

        match json["action"].as_str() {
            Some("input") => {
                let i = json["payload"].as_str().unwrap().to_string();
                sender.send(Message::Command(Command::Input(i))).unwrap();
            }

            Some("getView") => {
                sender.send(Message::Command(Command::GetView)).unwrap();
            }

            other => {
                eprintln!("invalid action: {other:?}");
            }
        }
    }

    sender.send(Message::StdinClosed).unwrap();
}

fn handle_process(
    master_fd: RawFd,
    input_rx: OwnedFd,
    sender: mpsc::Sender<Message>,
    child: unistd::Pid,
) {
    handle_pty(master_fd, input_rx, sender.clone());
    eprintln!("killing the child with HUP");
    unsafe { libc::kill(child.as_raw(), libc::SIGHUP) };
    eprintln!("waiting for child's exit status");
    let _ = wait::waitpid(child, None);
    let _ = sender.send(Message::ChildExited);
}

fn handle_pty(master_fd: RawFd, input_rx: OwnedFd, sender: mpsc::Sender<Message>) {
    const MASTER: mio::Token = mio::Token(0);
    const INPUT: mio::Token = mio::Token(1);
    const BUF_SIZE: usize = 128 * 1024;

    let mut poll = mio::Poll::new().unwrap();
    let mut events = mio::Events::with_capacity(128);
    let mut master_file = unsafe { File::from_raw_fd(master_fd) };
    let mut master_source = SourceFd(&master_fd);
    let input_fd = input_rx.as_raw_fd();
    let mut input_file = unsafe { File::from_raw_fd(input_fd) };
    let mut input_source = SourceFd(&input_fd);
    let mut buf = [0u8; BUF_SIZE];
    let mut input: Vec<u8> = Vec::with_capacity(BUF_SIZE);
    nbio::set_non_blocking(&input_fd).unwrap();
    nbio::set_non_blocking(&master_fd).unwrap();

    poll.registry()
        .register(&mut master_source, MASTER, mio::Interest::READABLE)
        .unwrap();

    poll.registry()
        .register(&mut input_source, INPUT, mio::Interest::READABLE)
        .unwrap();

    loop {
        if let Err(e) = poll.poll(&mut events, None) {
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            } else {
                panic!("{}", e);
            }
        }

        for event in events.iter() {
            match event.token() {
                MASTER => {
                    if event.is_readable() {
                        println!("master read");

                        while let Some(n) = nbio::read(&mut master_file, &mut buf).unwrap() {
                            if n > 0 {
                                sender
                                    .send(Message::Output(
                                        String::from_utf8_lossy(&buf[0..n]).to_string(),
                                    ))
                                    .unwrap();
                            } else {
                                println!("master read closed");
                                return;
                            }
                        }
                    }

                    if event.is_writable() {
                        println!("master write");

                        let mut buf: &[u8] = input.as_ref();

                        while let Some(n) = nbio::write(&mut master_file, buf).unwrap() {
                            buf = &buf[n..];

                            if buf.is_empty() {
                                break;
                            }
                        }

                        let left = buf.len();

                        if left == 0 {
                            input.clear();

                            poll.registry()
                                .reregister(&mut master_source, MASTER, mio::Interest::READABLE)
                                .unwrap();
                        } else {
                            input.drain(..input.len() - left);
                        }
                    }

                    if event.is_read_closed() {
                        eprintln!("master closed");
                        return;
                    }
                }

                INPUT => {
                    if event.is_readable() {
                        println!("input read");

                        while let Some(n) = nbio::read(&mut input_file, &mut buf).unwrap() {
                            println!("read some input! {n}");

                            if n > 0 {
                                input.extend_from_slice(&buf[0..n]);

                                poll.registry()
                                    .reregister(
                                        &mut master_source,
                                        MASTER,
                                        mio::Interest::READABLE | mio::Interest::WRITABLE,
                                    )
                                    .unwrap();
                            } else {
                                eprintln!("input read is empty");
                                return;
                            }
                        }
                    }

                    if event.is_read_closed() {
                        eprintln!("input closed");
                        return;
                    }
                }

                _ => (),
            }
        }
    }
}

fn process_messages(receiver: mpsc::Receiver<Message>, mut input: File) {
    let mut vt = avt::Vt::builder().size(80, 24).build();

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
