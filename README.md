# Scrooge
A server that proxy HTTP requests and stream the responses in chunks of a specific size.

The implementation is based on [hyper_reverse_proxy](https://docs.rs/hyper-reverse-proxy/).

## Usage

The available command line arguments:
```
USAGE:
    scrooge [OPTIONS] <CONFIG>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -h, --host <host>    Set host to bind to [default: [::1]]
    -p, --port <port>    Set port [default: 8080]

ARGS:
    <CONFIG>    Configuration file
```

### Configuration File

Sample configuration file:
```
upstream_url = "http://127.0.0.1:8888"
utf8_body_limit = "1MB"
```