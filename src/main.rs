const VERSION: &str = "0.1.0";

fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("--help") => print_help(),
        Some("--version") => print_version(),
        Some(arg) => {
            eprintln!("error: unexpected argument '{arg}'");
            eprintln!("try 'bazel-diff-targets --help' for usage");
            std::process::exit(2);
        }
        None => print_help(),
    }
}

fn print_help() {
    println!(
        "\
bazel-diff-targets {VERSION}

Compute impacted Bazel targets between two Git refs.

Usage:
  bazel-diff-targets [OPTIONS]

Options:
    --help      Print help
    --version   Print version
"
    );
}

fn print_version() {
    println!("bazel-diff-targets {VERSION}");
}
