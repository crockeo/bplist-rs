use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::str;

use crate::result::{Error, Result};
use crate::trailer::Trailer;

#[derive(Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Filler,
    Int(i64),
    Real(f64),
    // Date
    Data(Vec<u8>),
    Str(String),
    UID(Vec<u8>),
    Array(Vec<Vec<u8>>),
    // Set
    Dict(HashMap<Vec<u8>, Vec<u8>>),
}

pub struct ObjectTable(HashMap<u64, Value>);

impl ObjectTable {
    pub fn load(file: &mut File, trailer: &Trailer) -> Result<ObjectTable> {
        // NOTE: assumes we're already at the beginning of the object table, i.e. we've already
        // verified the header and loaded the trailer.

        let mut object_table = ObjectTable(HashMap::new());
        let mut byte_offset = 0u64;
        for _ in 0..trailer.num_objects {
            let (item, byte_width) = parse_item(file, trailer)?;
            object_table.0.insert(byte_offset, item);
            byte_offset += byte_width;
        }

        Ok(object_table)
    }
}

fn parse_item(file: &mut File, trailer: &Trailer) -> Result<(Value, u64)> {
    let mut marker = [0u8];
    let bytes_read = file.read(&mut marker)?;
    if bytes_read == 0 {
        return Err(Error::EOF);
    }

    let marker_high = (marker[0] & 0b11110000) >> 4;
    let marker_low = marker[0] & 0b00001111;

    match marker_high {
        marker::SINGLE => parse_single(marker_low),
        marker::INT => parse_int(file, marker_low),
        marker::REAL => parse_real(file, marker_low),
        marker::DATE => todo!("date"),
        marker::DATA => parse_data(file, trailer, marker_low),
        marker::ASCII_STR => parse_ascii_str(file, trailer, marker_low),
        marker::UTF16_STR => parse_utf16_str(file, trailer, marker_low),
        marker::UID => parse_uid(file, marker_low),
        marker::ARRAY => parse_array(file, trailer, marker_low),
        marker::SET => todo!("set"),
        marker::DICT => parse_dict(file, trailer, marker_low),

        _ => return Err(Error::InvalidFormat("unrecognized part")),
    }
}

fn parse_single(marker_low: u8) -> Result<(Value, u64)> {
    Ok((
        match marker_low {
            0b0000 => Value::Null,
            0b1000 => Value::Bool(false),
            0b0001 => Value::Bool(true),
            0b1111 => Value::Filler,
            _ => {
                return Err(Error::InvalidFormat("invalid single byte"));
            }
        },
        0,
    ))
}

fn parse_int(file: &mut File, marker_low: u8) -> Result<(Value, u64)> {
    let mut byte_count = 1usize;
    for _ in 0..marker_low {
        byte_count *= 2;
    }

    let mut bytes = vec![0; byte_count];
    file.read_exact(bytes.as_mut_slice())?;

    let mut n = 0i64;
    for byte in bytes.into_iter() {
        n = (n << 8) | (byte as i64);
    }

    Ok((Value::Int(n), byte_count as u64 + 1))
}

fn parse_real(file: &mut File, marker_low: u8) -> Result<(Value, u64)> {
    let mut byte_count = 1usize;
    for _ in 0..marker_low {
        byte_count *= 2;
    }

    let mut bytes = vec![0; byte_count];
    file.read_exact(bytes.as_mut_slice())?;

    let mut float_buf = [0u8; 8];
    for (i, byte) in bytes.into_iter().rev().enumerate() {
        // bail early if we have too many bytes--need to actually throw an error here
        if i >= 8 {
            break;
        }
        float_buf[7 - i] = byte;
    }

    Ok((
        Value::Real(f64::from_be_bytes(float_buf)),
        byte_count as u64 + 1,
    ))
}

fn parse_data(file: &mut File, trailer: &Trailer, marker_low: u8) -> Result<(Value, u64)> {
    let (length, byte_width) = read_length(file, trailer, marker_low)?;
    let mut buf = vec![0; length as usize];
    file.read_exact(buf.as_mut_slice())?;
    Ok((Value::Data(buf), length as u64 + byte_width + 1))
}

fn parse_ascii_str(file: &mut File, trailer: &Trailer, marker_low: u8) -> Result<(Value, u64)> {
    let (length, byte_width) = read_length(file, trailer, marker_low)?;
    let mut buf = vec![0; length as usize];
    file.read_exact(buf.as_mut_slice())?;
    Ok((
        Value::Str(as_utf8(&buf)?.to_owned()),
        length as u64 + byte_width + 1,
    ))
}

fn parse_utf16_str(file: &mut File, trailer: &Trailer, marker_low: u8) -> Result<(Value, u64)> {
    let (length, byte_width) = read_length(file, trailer, marker_low)?;
    let mut buf = vec![0; length as usize * 2];
    file.read_exact(buf.as_mut_slice())?;
    Ok((
        Value::Str(as_utf16(&buf)?),
        length as u64 * byte_width * 2 + 1,
    ))
}

fn parse_uid(file: &mut File, marker_low: u8) -> Result<(Value, u64)> {
    let mut buf = vec![0; (marker_low + 1) as usize];
    file.read_exact(buf.as_mut_slice())?;
    Ok((Value::UID(buf), marker_low as u64 + 2))
}

fn parse_array(file: &mut File, trailer: &Trailer, marker_low: u8) -> Result<(Value, u64)> {
    let (length, byte_width) = read_length(file, trailer, marker_low)?;
    let mut values = Vec::with_capacity(length as usize);
    for _ in 0..length {
        let mut objref = vec![0; trailer.object_ref_size as usize];
        file.read_exact(objref.as_mut_slice())?;
        values.push(objref);
    }
    Ok((
        Value::Array(values),
        length as u64 * trailer.object_ref_size as u64 * byte_width + 1,
    ))
}

fn parse_dict(file: &mut File, trailer: &Trailer, marker_low: u8) -> Result<(Value, u64)> {
    let (length, byte_width) = read_length(file, trailer, marker_low)?;
    let mut references = HashMap::new();
    for _ in 0..length {
        let mut keyref = vec![0; trailer.object_ref_size as usize];
        let mut objref = vec![0; trailer.object_ref_size as usize];

        file.read_exact(keyref.as_mut_slice())?;
        file.read_exact(objref.as_mut_slice())?;

        references.insert(keyref, objref);
    }

    Ok((
        Value::Dict(references),
        length as u64 * trailer.object_ref_size as u64 * byte_width * 2 + 1,
    ))
}

fn as_utf8(buf: &[u8]) -> Result<&str> {
    match str::from_utf8(buf) {
        Err(_) => Err(Error::EncodingError),
        Ok(x) => Ok(x),
    }
}

fn as_utf16(buf: &[u8]) -> Result<String> {
    if buf.len() % 2 != 0 {
        return Err(Error::InvalidFormat("utf16 buf must be even length"));
    }

    let mut combined_buf = vec![0; buf.len() / 2];
    for i in 0..buf.len() / 2 {
        combined_buf[i] = ((buf[2 * i] as u16) << 8) | (buf[2 * i + 1] as u16);
    }

    String::from_utf16(&combined_buf).map_err(|_| Error::InvalidFormat("invalid utf16 format"))
}

fn read_length(file: &mut File, trailer: &Trailer, marker_low: u8) -> Result<(i64, u64)> {
    if marker_low == 0b1111 {
        let (item, byte_width) = parse_item(file, trailer)?;
        if let Value::Int(n) = item {
            Ok((n, byte_width + 1))
        } else {
            Err(Error::InvalidFormat("invalid dict size"))
        }
    } else {
        Ok((marker_low as i64, 1))
    }
}

mod marker {
    pub const SINGLE: u8 = 0;
    pub const INT: u8 = 1;
    pub const REAL: u8 = 2;
    pub const DATE: u8 = 3;
    pub const DATA: u8 = 4;
    pub const ASCII_STR: u8 = 5;
    pub const UTF16_STR: u8 = 6;
    pub const UID: u8 = 8;
    pub const ARRAY: u8 = 10;
    pub const SET: u8 = 12;
    pub const DICT: u8 = 13;
}
