use nix::pty;
use nix::unistd::{self, ForkResult};
use std::{
    fs::File,
    io::Write,
    os::fd::{AsRawFd, FromRawFd, RawFd},
};
use nix::libc;
use std::ffi::{CString, NulError};
use std::io;
use std::sync::mpsc;
use std::thread;

enum Message {
    Output(String),
    Command(Command),
}

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
            handle_child(&["/bin/bash"]).unwrap();
            unreachable!();
        }
    }
}

fn handle_parent(master_fd: RawFd, child: unistd::Pid) {
    let (sender, receiver) = mpsc::channel::<Message>();
    let (rx, tx) = nix::unistd::pipe().unwrap();
    let mut input = unsafe { File::from_raw_fd(tx.as_raw_fd()) };

    let s1 = sender.clone();
    let h1 = thread::spawn(move || {
        for line in std::io::stdin().lines() {
            let json: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();

            match json["action"].as_str() {
                Some("input") => {
                    let i = json["payload"].as_str().unwrap().to_string();
                    s1.send(Message::Command(Command::Input(i))).unwrap();
                }

                Some("getView") => {
                    s1.send(Message::Command(Command::GetView)).unwrap();
                }

                _ => (),
            }
        }
    });

    let h2 = thread::spawn(move || {
        // TODO select / copy
        sender.send(Message::Output("".to_string())).unwrap();
    });

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

fn handle_child<S>(command: &[S]) -> io::Result<()>
where
    S: AsRef<str>,
{
    let command = command
        .iter()
        .map(|s| CString::new(s.as_ref()))
        .collect::<Result<Vec<CString>, NulError>>()
        .unwrap();

    unistd::execvp(&command[0], &command).unwrap();
    unsafe { libc::_exit(1) }
}
