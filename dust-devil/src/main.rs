use std::{env, process::exit};

mod args;
mod context;
mod logger;
mod server;
mod socks5;
mod users;
mod utils;

use args::*;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{}\n\nType 'dust-devil --help' for a help menu", err);
            exit(1);
        }
        Ok(arguments) => arguments,
    };

    let startup_args = match arguments {
        ArgumentsRequest::Version => {
            println!("{}", get_version_string());
            println!("Your mother's favorite socks5 proxy server");
            return;
        }
        ArgumentsRequest::Help => {
            println!("{}", get_help_string());
            return;
        }
        ArgumentsRequest::Run(startup_args) => startup_args,
    };

    let start_result = tokio::runtime::Builder::new_multi_thread().enable_all().build();

    match start_result {
        Ok(runtime) => runtime.block_on(server::run_server(startup_args)),
        Err(err) => eprintln!("Failed to start Tokio runtime: {err}"),
    }
}
