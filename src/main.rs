#[macro_use]
extern crate log;

use hyper::{Body, Request, Response, Server, Client, Uri, StatusCode};
use hyper::header::{HeaderMap, HeaderValue};
use futures::future::{self, Future};
use hyper::server::conn::AddrStream;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use std::net::IpAddr;
use std::str::FromStr;
use lazy_static::lazy_static;

type BoxFut = Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>;

fn is_hop_header(name: &str) -> bool {
    use unicase::Ascii;

    // A list of the headers, using `unicase` to help us compare without
    // worrying about the case, and `lazy_static!` to prevent reallocation
    // of the vector.
    lazy_static! {
        static ref HOP_HEADERS: Vec<Ascii<&'static str>> = vec![
            Ascii::new("Connection"),
            Ascii::new("Keep-Alive"),
            Ascii::new("Proxy-Authenticate"),
            Ascii::new("Proxy-Authorization"),
            Ascii::new("Te"),
            Ascii::new("Trailers"),
            Ascii::new("Transfer-Encoding"),
            Ascii::new("Upgrade"),
        ];
    }

    HOP_HEADERS.iter().any(|h| h == &name)
}

/// Returns a clone of the headers without the [hop-by-hop headers].
///
/// [hop-by-hop headers]: http://www.w3.org/Protocols/rfc2616/rfc2616-sec13.html
fn remove_hop_headers(headers: &HeaderMap<HeaderValue>) -> HeaderMap<HeaderValue> {
    let mut result = HeaderMap::new();
    for (k, v) in headers.iter() {
        if !is_hop_header(k.as_str()) {
            result.insert(k.clone(), v.clone());
        }
    }
    result
}

fn create_proxied_response<B>(mut response: Response<B>) -> Response<B> {
    *response.headers_mut() = remove_hop_headers(response.headers());
    response
}

fn forward_uri<B>(forward_url: &str, req: &Request<B>) -> Uri {
    let forward_uri = match req.uri().query() {
        Some(query) => format!("{}{}?{}", forward_url, req.uri().path(), query),
        None => format!("{}{}", forward_url, req.uri().path()),
    };

    Uri::from_str(forward_uri.as_str()).unwrap()
}

fn create_proxied_request<B>(client_ip: IpAddr, forward_url: &str, mut request: Request<B>) -> Request<B> {
    *request.headers_mut() = remove_hop_headers(request.headers());
    *request.uri_mut() = forward_uri(forward_url, &request);

    let x_forwarded_for_header_name = "x-forwarded-for";

    // Add forwarding information in the headers
    match request.headers_mut().entry(x_forwarded_for_header_name) {

        Ok(header_entry) => {
            match header_entry {
                hyper::header::Entry::Vacant(entry) => {
                    let addr = format!("{}", client_ip);
                    entry.insert(addr.parse().unwrap());
                },

                hyper::header::Entry::Occupied(mut entry) => {
                    let addr = format!("{}, {}", entry.get().to_str().unwrap(), client_ip);
                    entry.insert(addr.parse().unwrap());
                }
            }
        }

        // shouldn't happen...
        Err(_) => panic!("Invalid header name: {}", x_forwarded_for_header_name),
    }

    request
}

pub fn proxy_call(client_ip: IpAddr, forward_uri: &str, request: Request<Body>) -> BoxFut {

	let proxied_request = create_proxied_request(client_ip, forward_uri, request);

	let client = Client::new();
	let response = client.request(proxied_request).then(|response| {

		let proxied_response = match response {
            Ok(response) => create_proxied_response(response),
            Err(error) => {
                println!("Error: {}", error); // TODO: Configurable logging
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            },
        };


        future::ok(proxied_response)
	});

	Box::new(response)
}

fn main() {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 8080).into();

    let proxy_service = make_service_fn(|socket: &AddrStream| {
        // called every time a new socket connection is accepted!

        let remote_addr = socket.remote_addr();

        service_fn(move |req: Request<Body>| {
            // called after incoming socket connection was processed and loaded in high level structures

            let upstream_url = "http://localhost:8888";

            println!("Processing request ClientIP({}) -> {}", remote_addr.ip().to_string(), upstream_url);

            return proxy_call(remote_addr.ip(), upstream_url, req)
        })
    });

    let server = Server::bind(&addr)
        .serve(proxy_service)
        .map_err(|e| eprintln!("server error: {}", e));

    info!("Listening on http://{}", addr);
    hyper::rt::run(server);
}
