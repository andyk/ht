mod nbio;
use mio::unix::SourceFd;
use nix::libc;
use nix::pty;
use nix::sys::signal::{self, SigHandler, Signal};
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
    Output(String),
    Command(Command),
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
        ForkResult::Parent { child } => handle_parent(result.master.as_raw_fd(), child),

        ForkResult::Child => {
            handle_child(&["/bin/sh"]).unwrap();
            unreachable!();
        }
    }
}

fn handle_parent(master_fd: RawFd, child: unistd::Pid) {
    let (sender, receiver) = mpsc::channel::<Message>();
    let (input_rx, input_tx) = nix::unistd::pipe().unwrap();
    let input = unsafe { File::from_raw_fd(input_tx.as_raw_fd()) };
    let sender_1 = sender.clone();
    let sender_2 = sender.clone();

    thread::scope(|s| {
        s.spawn(move || read_stdin(sender_1));
        s.spawn(move || handle_master(master_fd, input_rx, sender_2));
        process_messages(receiver, input);
    });
}

fn handle_child<S>(command: &[S]) -> io::Result<()>
where
    S: AsRef<str>,
{
    let command = command
        .iter()
        .map(|s| CString::new(s.as_ref()))
        .collect::<Result<Vec<CString>, NulError>>()
        .unwrap();

    env::set_var("TERM", "xterm-256color");
    unsafe { signal::signal(Signal::SIGPIPE, SigHandler::SigDfl) }.unwrap();
    unistd::execvp(&command[0], &command).unwrap();
    unsafe { libc::_exit(1) }
}

fn read_stdin(sender: mpsc::Sender<Message>) {
    for line in std::io::stdin().lines() {
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

    println!("input closed!");
}

fn handle_master(master_fd: RawFd, input_rx: OwnedFd, sender: mpsc::Sender<Message>) {
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

                    // needed?
                    // if event.is_read_closed() {
                    //     poll.registry().deregister(&mut master_source).unwrap();
                    //     return;
                    // }
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
                                return;
                            }
                        }
                    }

                    // needed?
                    // if event.is_read_closed() {
                    //     poll.registry().deregister(&mut input_source).unwrap();
                    //     return;
                    // }
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
        }
    }
}
