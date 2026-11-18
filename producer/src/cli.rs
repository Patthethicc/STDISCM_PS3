//! Command-line arguments.

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Uploads videos to the media upload consumer")]
pub struct Cli {
    /// Number of producer threads (p)
    #[arg(short, long)]
    pub p: usize,

    /// Video folder for a thread; pass exactly p times, in thread order
    #[arg(short, long = "folder", required = true)]
    pub folders: Vec<PathBuf>,

    /// Consumer address, e.g. 192.168.1.10:50051
    #[arg(short, long)]
    pub consumer: String,
}

impl Cli {
    /// Checks that `p` matches the number of folders given.
    pub fn validate(&self) -> Result<(), String> {
        if self.p == 0 || self.folders.len() != self.p {
            return Err(format!(
                "-p is {} but {} --folder value(s) were given; they must match and be at least 1",
                self.p,
                self.folders.len()
            ));
        }
        Ok(())
    }

    /// Consumer address as a URL (`http://` is added when no scheme is given).
    pub fn endpoint(&self) -> String {
        if self.consumer.contains("://") {
            self.consumer.clone()
        } else {
            format!("http://{}", self.consumer)
        }
    }
}
