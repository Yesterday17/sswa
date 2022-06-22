use crate::args::Args;
use clap::Parser;
use clap_handler::Handler;

mod config;
mod args;
mod template;
mod ffmpeg;
mod context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Args::parse().run().await
}
