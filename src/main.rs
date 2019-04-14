extern crate config;

use futures::prelude::*;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Client as HyperClient, Request, Server,
};
use scrooge::{config::ProxyConfig, proxy_call, Client};
use std::{
    process,
    sync::Arc,
    path::PathBuf
};
use structopt::StructOpt;

/// Proxy HTTP requests and stream the responses in chunks of a specific size.
#[derive(StructOpt, Debug)]
struct Arguments {
    /// Set host to bind to
    #[structopt(short = "h", long = "host", default_value = "[::1]")]
    host: String,

    /// Set port
    #[structopt(short = "p", long = "port", default_value = "8080")]
    port: u32,

    /// Configuration file
    #[structopt(name = "CONFIG", parse(from_os_str))]
    config_path: PathBuf
}

fn main() {
    pretty_env_logger::init();

    let args = Arguments::from_args();

    let config = {
        let mut settings = config::Config::default();
        match settings.merge(config::File::from(args.config_path)) {
            Ok(_) => settings.try_into::<ProxyConfig>().unwrap(),
            Err(why) => {
                eprintln!("Error: {}", why);
                process::exit(1);
            }
        }
    };

    let max_chunk_size = match config.max_chunk_size_in_bytes() {
        Ok(v) => v,
        Err(why) => {
            eprintln!("Error: {}", why);
            process::exit(1);
        }
    };

    let addr = format!("{}:{}", args.host, args.port).parse().unwrap();

    println!("Server running at http://{}:{} chunking responses at {}", args.host, args.port, config.utf8_body_limit);

    let upstream_url = Arc::new(config.upstream_url.clone());
    let http_client = Arc::new(HyperClient::builder().keep_alive(true).build_http::<Body>());

    let proxy_service = make_service_fn(move |socket: &AddrStream| {
        // every time a new socket connection is accepted!
        let client = Arc::new(Client::new(
            upstream_url.clone(),
            max_chunk_size,
            socket.remote_addr().ip(),
        ));
        let http_client = http_client.clone();

        service_fn(move |req: Request<Body>| {
            // called after incoming socket connection was processed and loaded in high level structures
            proxy_call(http_client.clone(), client.clone(), req)
        })
    });

    let server = Server::bind(&addr)
        .serve(proxy_service)
        .map_err(|e| eprintln!("server error: {}", e));

    hyper::rt::run(server);
}
