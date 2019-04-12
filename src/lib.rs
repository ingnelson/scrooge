#[macro_use]
extern crate log;

use std::net::IpAddr;
use std::str;
use std::str::FromStr;

use futures::future::{self, Future};
use futures::prelude::*;
use hyper::header::{HeaderMap, HeaderValue};
use hyper::{Body, Client, Request, Response, StatusCode, Uri};
use lazy_static::lazy_static;

type BoxFut = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

#[derive(Clone, Default)]
pub struct Config {
    upstream_url: String,
    max_chunk_size: usize,
}

impl Config {
    pub fn new() -> Self {
        Config::default()
    }

    pub fn with_upstream(mut self, upstream_url: String) -> Self {
        self.upstream_url = upstream_url;
        self
    }

    pub fn with_max_chunk_size(mut self, size: usize) -> Self {
        self.max_chunk_size = size;
        self
    }
}

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

            // We remove this header since we want to send content chunked
            Ascii::new("Content-Length"),
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

fn create_proxied_response(original_resp: Response<Body>, max_chunk_size: usize) -> BoxFut {
    // We transform the response from upstream in a chunked response
    let mut chunked_response = Response::builder();

    for (k, v) in remove_hop_headers(original_resp.headers()).iter() {
        chunked_response.header(k, v);
    }

    // just to make sure we are going throgh this service
    chunked_response.header("x-proxy-by", "scrooge");

    Box::new(
        original_resp
            .into_body()
            .concat2()
            .from_err()
            .and_then(move |entire_body| {
                let chunks = entire_body
                    .into_bytes()
                    .chunks(max_chunk_size)
                    .map(|part| String::from_utf8(part.to_vec()).unwrap())
                    .collect::<Vec<_>>();

                let stream = futures::stream::iter_ok::<_, ::std::io::Error>(chunks);
                Ok(chunked_response.body(Body::wrap_stream(stream)).unwrap())
            }),
    )
}

fn forward_uri<B>(forward_url: &str, req: &Request<B>) -> Uri {
    let forward_uri = match req.uri().query() {
        Some(query) => format!("{}{}?{}", forward_url, req.uri().path(), query),
        None => format!("{}{}", forward_url, req.uri().path()),
    };

    Uri::from_str(forward_uri.as_str()).unwrap()
}

fn create_proxied_request(
    upstream_url: &str,
    client_ip: IpAddr,
    mut request: Request<Body>,
) -> Request<Body> {
    *request.headers_mut() = remove_hop_headers(request.headers());
    *request.uri_mut() = forward_uri(upstream_url, &request);

    let x_forwarded_for_header_name = "x-forwarded-for";

    // Add forwarding information in the headers
    match request.headers_mut().entry(x_forwarded_for_header_name) {
        Ok(header_entry) => match header_entry {
            hyper::header::Entry::Vacant(entry) => {
                let addr = format!("{}", client_ip);
                entry.insert(addr.parse().unwrap());
            }

            hyper::header::Entry::Occupied(mut entry) => {
                let addr = format!("{}, {}", entry.get().to_str().unwrap(), client_ip);
                entry.insert(addr.parse().unwrap());
            }
        },

        // shouldn't happen...
        Err(_) => panic!("Invalid header name: {}", x_forwarded_for_header_name),
    }

    request
}

pub fn proxy_call(config: &Config, client_ip: IpAddr, request: Request<Body>) -> BoxFut {
    let proxied_request = create_proxied_request(&config.upstream_url, client_ip, request);

    let max_chunk_size = config.max_chunk_size;

    info!(
        "Processing request ClientIP({}) -> {}",
        client_ip, config.upstream_url
    );

    let client = Client::new();
    let response = client.request(proxied_request).then(move |response| {
        match response {
            Ok(response) => create_proxied_response(response, max_chunk_size),
            Err(error) => {
                println!("Error: {}", error); // TODO: Configurable logging
                Box::new(future::ok(
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap(),
                ))
            }
        }
    });

    Box::new(response)
}
