use crate::args::Args;
use clap::Parser;
use anni_clap_handler::Handler;

mod config;
mod args;
mod template;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Args::parse().run().await
}
