use std::{env, process::exit};

mod args;
mod server;
mod socks5;
mod users;
mod utils;

use args::*;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{}", err);
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

    println!("Startup args: {startup_args:?}");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(server::run_server(startup_args));
}
