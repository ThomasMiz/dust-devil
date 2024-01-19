use std::{env, process::exit};

use tokio::task::LocalSet;

use crate::args::{get_help_string, get_version_string, ArgumentsRequest};

mod args;
mod client;
mod handle_requests;
mod sandstorm;
mod utils;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{err}\n\nType 'sandstorm --help' for a help menu");
            exit(1);
        }
        Ok(arguments) => arguments,
    };

    let startup_args = match arguments {
        ArgumentsRequest::Version => {
            println!("{}", get_version_string());
            println!("Makes the sand get everywhere (and that's very itchy)");
            return;
        }
        ArgumentsRequest::Help => {
            println!("{}", get_help_string());
            return;
        }
        ArgumentsRequest::Run(startup_args) => startup_args,
    };

    printlnif!(startup_args.verbose, "Starting up Tokio runtime");
    let start_result = tokio::runtime::Builder::new_current_thread().enable_all().build();

    match start_result {
        Ok(runtime) => LocalSet::new().block_on(&runtime, client::run_client(startup_args)),
        Err(err) => eprintln!("Failed to start Tokio runtime: {err}"),
    }
}
