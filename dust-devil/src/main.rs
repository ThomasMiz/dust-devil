use std::{env, process::exit};

mod args;
mod logging;
mod context;
mod server;
mod socks5;
mod users;
mod utils;

use args::*;

use std::io::Write;

fn main() {
    let mut hola = Vec::<u8>::with_capacity(0x2000);
    let res = writeln!(hola, "Pedro! 😍😍");
    println!("{res:?}");
    let _asd = writeln!(hola, "Nô mé jodás");
    let mut f = std::fs::File::create("loggy.txt").unwrap();
    println!("wrote: {:?}", f.write_all(&hola));
    drop(f);

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

    let start_result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();

    match start_result {
        Ok(runtime) => runtime.block_on(server::run_server(startup_args)),
        Err(err) => eprintln!("Failed to start Tokio runtime: {err}"),
    }
}
