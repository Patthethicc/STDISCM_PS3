use std::process::ExitCode;

use clap::Parser;
use producer::cli::Cli;
use producer::worker;

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = cli.validate() {
        eprintln!("error: {e}");
        return ExitCode::from(2);
    }

    let endpoint = cli.endpoint();
    let threads: Vec<_> = cli
        .folders
        .into_iter()
        .enumerate()
        .map(|(i, folder)| {
            let endpoint = endpoint.clone();
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime")
                    .block_on(worker::run(i + 1, folder, endpoint));
            })
        })
        .collect();

    for t in threads {
        let _ = t.join();
    }
    ExitCode::SUCCESS
}
