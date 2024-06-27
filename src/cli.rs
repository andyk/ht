use anyhow::bail;
use clap::Parser;
use nix::pty;
use std::{fmt::Display, net::SocketAddr, ops::Deref, str::FromStr};

#[derive(Debug, Parser)]
#[clap(version, about)]
#[command(name = "ht")]
pub struct Cli {
    /// Terminal size
    #[arg(long, value_name = "COLSxROWS", default_value = Some("120x40"))]
    pub size: Size,

    /// Command to run inside the terminal
    #[arg(default_value = "bash")]
    pub command: Vec<String>,

    /// Enable HTTP server
    #[arg(short, long, default_missing_value = "127.0.0.1:0", num_args = 0..=1)]
    pub listen_addr: Option<SocketAddr>,
}

impl Cli {
    pub fn new() -> Self {
        Cli::parse()
    }
}

#[derive(Debug, Clone)]
pub struct Size(pty::Winsize);

impl Size {
    pub fn cols(&self) -> usize {
        self.0.ws_col as usize
    }

    pub fn rows(&self) -> usize {
        self.0.ws_row as usize
    }
}

impl FromStr for Size {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        match s.split_once('x') {
            Some((cols, rows)) => {
                let cols: u16 = cols.parse()?;
                let rows: u16 = rows.parse()?;

                let winsize = pty::Winsize {
                    ws_col: cols,
                    ws_row: rows,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                Ok(Size(winsize))
            }

            None => {
                bail!("invalid size format: {s}");
            }
        }
    }
}

impl Deref for Size {
    type Target = pty::Winsize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.0.ws_col, self.0.ws_row)
    }
}
