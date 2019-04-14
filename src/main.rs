#[macro_use]
extern crate log;

use futures::prelude::*;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Server,
};
use lazy_static::lazy_static;
use scrooge::{config::ProxyConfig, proxy_call, Client};
use std::process;

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 8080).into();

    lazy_static! {
        static ref CONFIG: ProxyConfig =
            ProxyConfig::new(String::from("http://127.0.0.1:8888"), String::from("64KB"));
    }

    let max_chunk_size = match CONFIG.max_chunk_size_in_bytes() {
        Ok(v) => v,
        Err(why) => {
            eprintln!("Error: {}", why);
            process::exit(1);
        }
    };

    let proxy_service = make_service_fn(move |socket: &AddrStream| {
        // every time a new socket connection is accepted!
        let client = Client::new(
            CONFIG.upstream_url(),
            max_chunk_size,
            socket.remote_addr().ip(),
        );

        service_fn(move |req: Request<Body>| {
            // called after incoming socket connection was processed and loaded in high level structures
            proxy_call(client.clone(), req)
        })
    });

    let server = Server::bind(&addr)
        .serve(proxy_service)
        .map_err(|e| eprintln!("server error: {}", e));

    info!("Listening on http://{}", addr);
    hyper::rt::run(server);
}
