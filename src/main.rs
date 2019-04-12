#[macro_use]
extern crate log;

use futures::prelude::*;
use hyper::server::conn::AddrStream;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use hyper::{Body, Request, Server};

use scrooge;
use scrooge::Config;

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 8080).into();
    let config = Config::new(String::from("http://127.0.0.1:8888"), "64B".parse().unwrap());

    let proxy_service = make_service_fn(move |socket: &AddrStream| {
        // called every time a new socket connection is accepted!
        let config = config.clone();
        let remote_addr = socket.remote_addr();

        service_fn(move |req: Request<Body>| {
            // called after incoming socket connection was processed and loaded in high level structures
            scrooge::proxy_call(&config, remote_addr.ip(), req)
        })
    });

    let server = Server::bind(&addr)
        .serve(proxy_service)
        .map_err(|e| eprintln!("server error: {}", e));

    info!("Listening on http://{}", addr);
    hyper::rt::run(server);
}
