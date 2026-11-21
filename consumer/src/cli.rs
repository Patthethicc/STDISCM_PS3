use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "consumer", about = "Upload consumer with leaky-bucket queue")]
pub struct Cli {
    /// Number of worker tasks (c)
    #[arg(short = 'c', long = "concurrency", default_value_t = 4)]
    pub concurrency: usize,

    /// Maximum queue capacity (q)
    #[arg(short = 'q', long = "queue-capacity", default_value_t = 16)]
    pub queue_capacity: usize,

    /// gRPC bind address
    #[arg(long, default_value = "0.0.0.0:50051")]
    pub bind: String,

    /// Directory where finished videos are stored
    #[arg(long, default_value = "./uploads")]
    pub uploads_dir: String,

    #[arg(long, default_value = "./data")]
    pub data_dir: String,
}