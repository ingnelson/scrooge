use human_size::{Byte, SpecificSize};
use serde::Deserialize;
use std::error::Error;

#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    upstream_url: String,
    utf8_body_limit: String,
}

impl ProxyConfig {
    pub fn new(upstream_url: String, utf8_body_limit: String) -> Self {
        Self {
            upstream_url,
            utf8_body_limit,
        }
    }

    pub fn upstream_url(&self) -> &String {
        &self.upstream_url
    }

    pub fn max_chunk_size_in_bytes(&self) -> Result<usize, Box<dyn Error>> {
        Ok(self.utf8_body_limit.parse::<SpecificSize<Byte>>()?.value() as usize)
    }
}
