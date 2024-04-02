use std::{fs::File, io::Write, os::fd::{AsRawFd, FromRawFd, RawFd}};
use nix::pty;
use nix::unistd::{self, ForkResult};
use nix::libc;
use std::ffi::{CString, NulError};
use std::io;

fn main() {
    let (rx, tx) = nix::unistd::pipe().unwrap();
    let mut input = unsafe { File::from_raw_fd(tx.as_raw_fd()) };

    let winsize = pty::Winsize {
        ws_col: 80,
        ws_row: 24,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let result = unsafe { pty::forkpty(Some(&winsize), None) }.unwrap();

    match result.fork_result {
        ForkResult::Parent { child } => handle_parent(result.master.as_raw_fd(), child, input),

        ForkResult::Child => {
            handle_child(&["/bin/bash"]).unwrap();
            unreachable!();
        }
    }
}

fn handle_parent(master_fd: RawFd, child: unistd::Pid, mut input: File) {
    let mut vt = avt::Vt::builder().size(80, 24).build();

    for line in std::io::stdin().lines() {
        let json: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();

        match json["action"].as_str() {
            Some("input") => {
                input.write_all(json["payload"].as_str().unwrap().as_bytes()).unwrap();
            }

            Some("getView") => {
                let text = vt
                    .lines()
                    .iter()
                    .map(|l| l.text())
                    .collect::<Vec<_>>()
                    .join("\n");

                let resp = serde_json::json!({ "view": text });
                println!("{}", serde_json::to_string(&resp).unwrap());
            }

            _ => (),
        }
    }
}

fn handle_child<S>(command: &[S]) -> io::Result<()> where S: AsRef<str> {
    let command = command
        .iter()
        .map(|s| CString::new(s.as_ref()))
        .collect::<Result<Vec<CString>, NulError>>().unwrap();

    unistd::execvp(&command[0], &command).unwrap();
    unsafe { libc::_exit(1) }
}
