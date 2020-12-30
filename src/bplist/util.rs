use std::str;

use super::result::{Error, Result};

pub fn from_be_bytes(bytes: &Vec<u8>) -> u64 {
    if bytes.len() > 8 {
        panic!("oops better error handling here");
    }

    let mut register = 0u64;
    for byte in bytes.into_iter() {
        register = (register << 8) | *byte as u64;
    }
    register
}

pub fn as_utf8(buf: &[u8]) -> Result<&str> {
    match str::from_utf8(buf) {
        Err(_) => Err(Error::EncodingError),
        Ok(x) => Ok(x),
    }
}

pub fn as_utf16(buf: &[u8]) -> Result<String> {
    if buf.len() % 2 != 0 {
        return Err(Error::InvalidFormat("utf16 buf must be even length"));
    }

    let mut combined_buf = vec![0; buf.len() / 2];
    for i in 0..buf.len() / 2 {
        combined_buf[i] = ((buf[2 * i] as u16) << 8) | (buf[2 * i + 1] as u16);
    }

    String::from_utf16(&combined_buf).map_err(|_| Error::EncodingError)
}
