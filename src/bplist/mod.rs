mod reference_table;
mod result;
mod trailer;
mod util;

use std::fmt;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::str;

use reference_table::ReferenceTable;
pub use result::{Error, Result};
use trailer::Trailer;

/* taken from https://opensource.apple.com/source/CF/CF-550/CFBinaryPList.c for reference while

HEADER
    magic number ("bplist")
    file format version

OBJECT TABLE
    variable-sized objects

    Object Formats (marker byte followed by additional info in some cases)
    null	0000 0000
    bool	0000 1000			// false
    bool	0000 1001			// true
    fill	0000 1111			// fill byte
    int	0001 nnnn	...		// # of bytes is 2^nnnn, big-endian bytes
    real	0010 nnnn	...		// # of bytes is 2^nnnn, big-endian bytes
    date	0011 0011	...		// 8 byte float follows, big-endian bytes
    data	0100 nnnn	[int]	...	// nnnn is number of bytes unless 1111 then int count follows, followed by bytes
    string	0101 nnnn	[int]	...	// ASCII string, nnnn is # of chars, else 1111 then int count, then bytes
    string	0110 nnnn	[int]	...	// Unicode string, nnnn is # of chars, else 1111 then int count, then big-endian 2-byte uint16_t
        0111 xxxx			// unused
    uid	1000 nnnn	...		// nnnn+1 is # of bytes
        1001 xxxx			// unused
    array	1010 nnnn	[int]	objref*	// nnnn is count, unless '1111', then int count follows
        1011 xxxx			// unused
    set	1100 nnnn	[int]	objref* // nnnn is count, unless '1111', then int count follows
    dict	1101 nnnn	[int]	keyref* objref*	// nnnn is count, unless '1111', then int count follows
        1110 xxxx			// unused
        1111 xxxx			// unused

OFFSET TABLE
    list of ints, byte size of which is given in trailer
    -- these are the byte offsets into the file
    -- number of these is in the trailer

TRAILER
    byte size of offset ints in offset table
    byte size of object refs in arrays and dicts
    number of offsets in offset table (also is number of objects)
    element # in offset table which is top level object
    offset table offset

*/

pub enum BPList {
    Null,
    Bool(bool),
    Filler,
    Int(i64),
    Real(f64),
    // Date
    Data(Vec<u8>),
    Str(String),
    UID(Vec<u8>),
    Array(Vec<Box<BPList>>),
    // Set
    Dict(Vec<(Box<BPList>, Box<BPList>)>),
}

impl Debug for BPList {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        self.print(fmt, 0)
    }
}

impl PartialEq for BPList {
    fn eq(&self, other: &Self) -> bool {
        use BPList::*;

        match (self, other) {
            (Null, Null) => true,
            (Bool(b1), Bool(b2)) => b1 == b2,
            (Filler, Filler) => true,
            (Int(i1), Int(i2)) => i1 == i2,
            (Data(d1), Data(d2)) => d1 == d2,
            (Str(s1), Str(s2)) => s1 == s2,
            (UID(u1), UID(u2)) => u1 == u2,

            // assume no equality for: Real, Array, Dict.
            _ => false,
        }
    }
}

impl BPList {
    pub fn load(file: &mut File) -> Result<BPList> {
        // ensuring this is the right format
        let mut magic_buf = [0; 8];
        file.read_exact(&mut magic_buf)?;
        let magic_buf_str = match str::from_utf8(&magic_buf) {
            Err(_) => return Err(Error::EncodingError),
            Ok(x) => x,
        };
        if magic_buf_str != "bplist00" {
            return Err(Error::InvalidFormat("invalid magic string"));
        }

        // get the necessary information to load the object table
        let object_table_pos = file.seek(SeekFrom::Current(0))?;

        file.seek(SeekFrom::End(-32))?;
        let trailer = Trailer::load(file)?;

        file.seek(SeekFrom::Start(trailer.offset_table_start))?;
        let reference_table = ReferenceTable::load(file, &trailer)?;

        // recursively populating the value
        file.seek(SeekFrom::Start(object_table_pos))?;
        BPList::load_item(file, &trailer, &reference_table)
    }

    fn load_item(
        file: &mut File,
        trailer: &Trailer,
        reference_table: &ReferenceTable,
    ) -> Result<BPList> {
        let mut marker = [0u8];
        let bytes_read = file.read(&mut marker)?;
        if bytes_read == 0 {
            return Err(Error::EOF);
        }

        let marker_high = (marker[0] & 0b11110000) >> 4;
        let marker_low = marker[0] & 0b00001111;

        match marker_high {
            // simple types
            marker::SINGLE => load_single(marker_low),
            marker::INT => load_int(file, marker_low),
            marker::REAL => load_real(file, marker_low),
            marker::DATE => todo!("date"),
            marker::DATA => load_data(file, trailer, reference_table, marker_low),
            marker::ASCII_STR => load_ascii_str(file, trailer, reference_table, marker_low),
            marker::UTF16_STR => load_utf16_str(file, trailer, reference_table, marker_low),
            marker::UID => load_uid(file, marker_low),

            // complex types
            marker::ARRAY => load_array(file, trailer, reference_table, marker_low),
            marker::SET => todo!("set"),
            marker::DICT => load_dict(file, trailer, reference_table, marker_low),

            x => {
                println!("{}", x);
                Err(Error::InvalidFormat("unrecognized part"))
            }
        }
    }

    pub fn print(&self, fmt: &mut Formatter, depth: u64) -> fmt::Result {
        match self {
            BPList::Null => write!(fmt, "null"),
            BPList::Bool(b) => write!(fmt, "{}", b),
            BPList::Filler => write!(fmt, "filler"),
            BPList::Int(i) => write!(fmt, "{}", i),
            BPList::Real(i) => write!(fmt, "{}", i),
            BPList::Data(bytes) => {
                write!(fmt, "[ ")?;
                for byte in bytes.into_iter() {
                    write!(fmt, "{} ", byte)?;
                }
                write!(fmt, "]")
            }
            BPList::Str(s) => write!(fmt, "{:?}", s),
            BPList::UID(bytes) => {
                write!(fmt, "[ ")?;
                for byte in bytes.into_iter() {
                    write!(fmt, "{} ", byte)?;
                }
                write!(fmt, "]")
            }
            BPList::Array(array) => {
                writeln!(fmt, "[ ")?;

                for item in array.into_iter() {
                    print_depth(fmt, depth + 1)?;
                    item.print(fmt, depth + 1)?;
                    writeln!(fmt, ",")?;
                }
                print_depth(fmt, depth)?;
                write!(fmt, "]")
            }
            BPList::Dict(array) => {
                writeln!(fmt, "{{")?;

                for (key, object) in array.into_iter() {
                    print_depth(fmt, depth + 1)?;
                    key.print(fmt, depth + 1)?;
                    write!(fmt, " -> ")?;
                    object.print(fmt, depth + 1)?;
                    writeln!(fmt, ",")?;
                }
                print_depth(fmt, depth)?;
                write!(fmt, "}}")
            }
        }
    }

    pub fn get<'a>(&'a self, lookup_key: BPList) -> Result<&'a BPList> {
        use BPList::*;

        match (self, &lookup_key) {
            // (Array(items), Int(idx)) => items.get(idx).ok_or(Error::NotFound),

            (Dict(items), lookup_key) => {
                for (key, value) in items.into_iter() {
                    if key.as_ref() == lookup_key {
                        return Ok(value);
                    }
                }
                Err(Error::NotFound)
            }

            _ => Err(Error::NotFound),
        }
    }

    pub fn gets<'a>(&'a self, lookup_key: &str) -> Result<&'a BPList> {
        self.get(BPList::Str(lookup_key.to_owned()))
    }

    pub fn geti<'a>(&'a self, lookup_key: usize) -> Result<&'a BPList> {
        self.get(BPList::Int(lookup_key as i64))
    }
}

fn print_depth(fmt: &mut Formatter, depth: u64) -> fmt::Result {
    for _ in 0..depth {
        write!(fmt, "  ")?;
    }
    Ok(())
}

fn load_single(marker_low: u8) -> Result<BPList> {
    Ok(match marker_low {
        0b0000 => BPList::Null,
        0b1000 => BPList::Bool(false),
        0b1001 => BPList::Bool(true),
        0b1111 => BPList::Filler,
        _ => {
            return Err(Error::InvalidFormat("invalid single byte"));
        }
    })
}

fn load_int(file: &mut File, marker_low: u8) -> Result<BPList> {
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

    Ok(BPList::Int(n))
}

fn load_real(file: &mut File, marker_low: u8) -> Result<BPList> {
    let mut byte_count = 1usize;
    for _ in 0..marker_low {
        byte_count *= 2;
    }

    let mut bytes = vec![0; byte_count];
    file.read_exact(bytes.as_mut_slice())?;

    let mut float_buf = [0u8; 8];
    for (i, byte) in bytes.into_iter().rev().enumerate() {
        // TODO
        // bail early if we have too many bytes--need to actually throw an error here
        if i >= 8 {
            break;
        }
        float_buf[7 - i] = byte;
    }

    Ok(BPList::Real(f64::from_be_bytes(float_buf)))
}

fn load_data(
    file: &mut File,
    trailer: &Trailer,
    reference_table: &ReferenceTable,
    marker_low: u8,
) -> Result<BPList> {
    let length = load_length(file, trailer, reference_table, marker_low)?;
    let mut buf = vec![0; length as usize];
    file.read_exact(buf.as_mut_slice())?;
    Ok(BPList::Data(buf))
}

fn load_ascii_str(
    file: &mut File,
    trailer: &Trailer,
    reference_table: &ReferenceTable,
    marker_low: u8,
) -> Result<BPList> {
    let length = load_length(file, trailer, reference_table, marker_low)?;
    let mut buf = vec![0; length as usize];
    file.read_exact(buf.as_mut_slice())?;
    Ok(BPList::Str(util::as_utf8(&buf)?.to_owned()))
}

fn load_utf16_str(
    file: &mut File,
    trailer: &Trailer,
    reference_table: &ReferenceTable,
    marker_low: u8,
) -> Result<BPList> {
    let length = load_length(file, trailer, reference_table, marker_low)?;
    let mut buf = vec![0; length as usize * 2];
    file.read_exact(buf.as_mut_slice())?;
    Ok(BPList::Str(util::as_utf16(&buf)?))
}

fn load_uid(file: &mut File, marker_low: u8) -> Result<BPList> {
    let mut buf = vec![0; (marker_low + 1) as usize];
    file.read_exact(buf.as_mut_slice())?;
    Ok(BPList::UID(buf))
}

fn load_array(
    file: &mut File,
    trailer: &Trailer,
    reference_table: &ReferenceTable,
    marker_low: u8,
) -> Result<BPList> {
    let length = load_length(file, trailer, reference_table, marker_low)?;

    let mut ref_buf = vec![0; trailer.object_ref_size as usize];
    let mut refs = Vec::new();
    for _ in 0..length {
        file.read_exact(ref_buf.as_mut_slice())?;
        refs.push(util::from_be_bytes(&ref_buf));
    }

    let mut objs = Vec::new();
    for objref in refs.into_iter() {
        seek_ref(file, reference_table, objref)?;
        objs.push(Box::new(BPList::load_item(file, trailer, reference_table)?));
    }

    Ok(BPList::Array(objs))
}

fn load_dict(
    file: &mut File,
    trailer: &Trailer,
    reference_table: &ReferenceTable,
    marker_low: u8,
) -> Result<BPList> {
    let length = load_length(file, trailer, reference_table, marker_low)?;

    let mut ref_buf = vec![0; trailer.object_ref_size as usize];
    let mut keyrefs = Vec::new();
    let mut objrefs = Vec::new();
    for _ in 0..length {
        file.read_exact(ref_buf.as_mut_slice())?;
        let keyref = util::from_be_bytes(&ref_buf);
        keyrefs.push(keyref);
    }

    for _ in 0..length {
        file.read_exact(ref_buf.as_mut_slice())?;
        let objref = util::from_be_bytes(&ref_buf);
        objrefs.push(objref);
    }

    let mut objs = Vec::new();
    for (keyref, objref) in keyrefs.into_iter().zip(objrefs.into_iter()) {
        seek_ref(file, reference_table, keyref)?;
        let key = BPList::load_item(file, trailer, reference_table)?;

        seek_ref(file, reference_table, objref)?;
        let obj = BPList::load_item(file, trailer, reference_table)?;

        objs.push((Box::new(key), Box::new(obj)));
    }

    Ok(BPList::Dict(objs))
}

fn load_length(
    file: &mut File,
    trailer: &Trailer,
    reference_table: &ReferenceTable,
    marker_low: u8,
) -> Result<i64> {
    if marker_low == 0b1111 {
        let item = BPList::load_item(file, trailer, reference_table)?;
        if let BPList::Int(n) = item {
            Ok(n)
        } else {
            Err(Error::InvalidFormat("invalid dict size"))
        }
    } else {
        Ok(marker_low as i64)
    }
}

fn seek_ref(file: &mut File, reference_table: &ReferenceTable, objref: u64) -> Result<u64> {
    let offset = reference_table.get(&objref).ok_or(Error::NotFound)?;
    Ok(file.seek(SeekFrom::Start(offset))?)
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
