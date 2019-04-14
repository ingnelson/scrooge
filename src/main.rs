#[macro_use]
extern crate log;

extern crate config;

use futures::prelude::*;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Server,
};
use scrooge::{config::ProxyConfig, proxy_call, Client};
use std::sync::Arc;
use std::process;

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 8080).into();
    let config = {
        let mut settings = config::Config::default();
        settings.merge(config::File::with_name("config")).unwrap();
        settings.try_into::<ProxyConfig>().unwrap()
    };

    let max_chunk_size = match config.max_chunk_size_in_bytes() {
        Ok(v) => v,
        Err(why) => {
            eprintln!("Error: {}", why);
            process::exit(1);
        }
    };

    let upstream_url = Arc::new(config.upstream_url().clone());

    let proxy_service = make_service_fn(move |socket: &AddrStream| {
        // every time a new socket connection is accepted!
        let client = Arc::new(Client::new(
            upstream_url.clone(),
            max_chunk_size,
            socket.remote_addr().ip(),
        ));

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
