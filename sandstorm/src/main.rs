use std::{env, process::exit};

use crate::args::{ArgumentsRequest, get_version_string, get_help_string};

mod args;
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

    println!("Startup args: {startup_args:?}");

    /*printlnif!(startup_args.verbose, "Starting up Tokio runtime");
    let start_result = tokio::runtime::Builder::new_multi_thread().enable_all().build();

    printlnif!(startup_args.verbose, "Entering runtime");
    match start_result {
        Ok(_runtime) => {},
        Err(err) => eprintln!("Failed to start Tokio runtime: {err}"),
    }*/
}
