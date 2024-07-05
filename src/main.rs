use clap::Parser;
use crabtrap::Config;
use std::env;
use std::ffi::CString;

#[derive(Parser)]
struct Cli {
    /// The path to the config file
    #[arg(long)]
    config: Option<std::path::PathBuf>,
    /// The target executable
    target: String,
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
    let config = args.config.map_or_else(Config::new, Config::from_file);

    println!(
        "{:?}",
        crabtrap::execute(
            &CString::new(args.target).unwrap(),
            &c_args.iter().map(|s| s.as_c_str()).collect::<Vec<_>>(),
            &c_env.iter().map(|s| s.as_c_str()).collect::<Vec<_>>(),
            &config,
        )
    );
}
