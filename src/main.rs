use clap::Parser;
use crabtrap::Config;
use std::env;
use std::ffi::CString;

#[derive(Parser)]
struct Cli {
    /// The target executable
    target: String,
    /// The path to the config file
    config: std::path::PathBuf,
    // Additional arguments
    args: Vec<String>,
}

fn main() {
    let args = Cli::parse();
    let c_args = args
        .args
        .into_iter()
        .map(|arg| CString::new(arg).unwrap())
        .collect::<Vec<_>>();
    let c_env = env::vars()
        .map(|(key, val)| CString::new(format!("{key}={val}")).unwrap())
        .collect::<Vec<_>>();

    println!(
        "{:?}",
        crabtrap::execute(
            &CString::new(args.target).unwrap(),
            &c_args.iter().map(|s| s.as_c_str()).collect::<Vec<_>>(),
            &c_env.iter().map(|s| s.as_c_str()).collect::<Vec<_>>(),
            &Config::from_file(args.config),
        )
    );
}
