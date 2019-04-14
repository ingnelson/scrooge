use human_size::{Byte, SpecificSize, ParsingError};
use serde::Deserialize;
use std::{ fmt, error::Error };

#[derive(Debug, Deserialize)]
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

    pub fn max_chunk_size_in_bytes(&self) -> Result<usize, ScroogeError> {
        match self.utf8_body_limit.parse::<SpecificSize<Byte>>() {
            Ok(v) => Ok(v.value() as usize),
            Err(why) => Err(ScroogeError::InvalidSizeFormat(&self.utf8_body_limit, why))
        }
    }
}

#[derive(Debug)]
pub enum ScroogeError<'a> {
    InvalidSizeFormat(&'a str, ParsingError)
}

impl<'a> fmt::Display for ScroogeError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScroogeError::InvalidSizeFormat(value, why) => {
                match why {
                   ParsingError::EmptyInput => write!(f, "Body size limit is required to be defined, found empty value."),
                   ParsingError::InvalidMultiple => write!(f, "Invalid multiple used in {}. Valid multiples are: B, kB, MB, GB, TB.", value),
                   ParsingError::InvalidValue => write!(f, "Invalid number value {}.", value),
                   ParsingError::MissingMultiple => write!(f, "Missing multiple in {}. Valid multiples are: B, kB, MB, GB, TB.", value),
                   ParsingError::MissingValue => write!(f, "Missing body size value in {}.", value)
                }
            }
        }
    }
}

impl<'a> Error for ScroogeError<'a> {}