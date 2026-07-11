mod args;
mod error;

use clap::Parser;

fn main() {
    let _args = args::Args::parse();
}
