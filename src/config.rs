use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "zk-ui", about = "ZooKeeper Visualization Tool")]
pub struct Cli {
    /// ZooKeeper host:port
    #[arg(long, default_value = "127.0.0.1:2181")]
    pub connect: String,

    /// Connection timeout in ms
    #[arg(long, default_value = "5000")]
    pub timeout: u64,
}
