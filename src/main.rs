extern crate config;

#[macro_use]
extern crate log;

use futures::prelude::*;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Client as HyperClient, Request, Server,
};
use scrooge::{config::ProxyConfig, proxy_call, Client};
use std::{path::PathBuf, process, sync::Arc};
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
    config_path: PathBuf,
}

macro_rules! maybe {
    ($e:expr) => {
        $e.unwrap_or_else(|why| {
            eprintln!("Error: {}", why);
            process::exit(1);
        })
    };
}

fn main() {
    pretty_env_logger::init();

    let args = Arguments::from_args();

    let config = maybe!(config::Config::default()
        .merge(config::File::from(args.config_path))
        .and_then(|v| v.clone().try_into::<ProxyConfig>()));

    let max_chunk_size = maybe!(config.max_chunk_size_in_bytes());
    let addr = maybe!(format!("{}:{}", args.host, args.port).parse());
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

        debug!("New connection ClientIP({})", client.ip);

        service_fn(move |req: Request<Body>| {
            debug!("New request from ClientIP({}) -> {} {}", client.ip, req.method(), req.uri().path());
            // called after incoming socket connection was processed and loaded in high level structures
            proxy_call(http_client.clone(), client.clone(), req)
        })
    });

    let server = maybe!(Server::try_bind(&addr))
        .serve(proxy_service)
        .map_err(|why| {
            eprintln!("Error: {}", why);
            process::exit(1);
        });

    println!(
        "Server running at http://{}:{} chunking responses at {}",
        args.host, args.port, config.utf8_body_limit
    );

    hyper::rt::run(server);
}
