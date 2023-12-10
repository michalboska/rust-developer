use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about, long_about)]
pub struct Cli {
    pub hostname: Option<String>,
    pub port: Option<u16>,
    pub web_port: Option<u16>,

    #[command(subcommand)]
    pub mode: Modes,
}

#[derive(Subcommand)]
pub enum Modes {
    CLIENT,
    SERVER,
}
