use crate::args::Args;
use clap::Parser;
use clap_handler::Handler;

mod args;
mod config;
mod context;
mod ffmpeg;
mod template;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Args::parse().run().await
}
