use std::{env, process::exit};

use crate::args::{get_help_string, get_version_string, ArgumentsRequest};

mod args;
mod context;
mod logger;
mod messaging;
mod sandstorm;
mod server;
mod socks5;
mod users;
mod utils;

#[cfg(test)]
mod tests;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{err}\n\nType 'dust-devil --help' for a help menu");
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

    printlnif!(startup_args.verbose, "Starting up Tokio runtime");
    let start_result = tokio::runtime::Builder::new_multi_thread().enable_all().build();

    match start_result {
        Ok(runtime) => runtime.block_on(server::run_server(startup_args)),
        Err(err) => eprintln!("Failed to start Tokio runtime: {err}"),
    }
}
