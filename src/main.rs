mod args;
mod error;
mod git;

use clap::Parser;

fn main() {
    let _args = args::Args::parse();
}
