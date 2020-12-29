use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::str;

mod object_table;
mod reference_table;
mod result;
mod trailer;
mod util;

/*
 * taken from https://opensource.apple.com/source/CF/CF-550/CFBinaryPList.c for reference while
 * implementing this project.
 *
 * other resources:
 *   - https://medium.com/@karaiskc/understanding-apples-binary-property-list-format-281e6da00dbd
 *   - https://opensource.apple.com/source/CF/CF-550/CFBinaryPList.c
 *   - https://opensource.apple.com/source/CF/CF-550/ForFoundationOnly.h

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

fn main() -> result::Result<()> {
    let mut file = File::open("test.ichat")?;

    // ensuring this is the right format
    let mut magic_buf = [0; 8];
    file.read_exact(&mut magic_buf)?;
    let magic_buf_str = match str::from_utf8(&magic_buf) {
        Err(_) => return Err(result::Error::EncodingError),
        Ok(x) => x,
    };
    if magic_buf_str != "bplist00" {
        return Err(result::Error::InvalidFormat("invalid magic string"));
    }

    let current_pos = file.seek(SeekFrom::Current(0))?;
    let trailer = trailer::Trailer::load(&mut file, current_pos)?;

    let object_table = object_table::ObjectTable::load(&mut file, &trailer)?;
    let reference_table = reference_table::ReferenceTable::load(&mut file, &trailer)?;

    recursively_print(&object_table, &reference_table, 8, 0)?;

    Ok(())
}

fn recursively_print(
    object_table: &object_table::ObjectTable,
    reference_table: &reference_table::ReferenceTable,
    offset: u64,
    depth: u64,
) -> result::Result<()> {
    match &object_table[&offset] {
        object_table::Value::Array(values) => {
            println!("[");
            for objref in values.into_iter() {
                print_depth(depth + 1);
                recursively_print(object_table, reference_table, reference_table[&objref], depth + 1)?;
                println!();
            }
            print_depth(depth);
            print!("]");
            Ok(())
        },
        object_table::Value::Dict(map) => {
            println!("{{");
            for (keyref, objref) in map.into_iter() {
                print_depth(depth + 1);
                recursively_print(object_table, reference_table, reference_table[keyref], depth + 1)?;
                print!(" -> ");
                recursively_print(object_table, reference_table, reference_table[objref], depth + 1)?;
                println!();
            }
            print_depth(depth);
            println!("}}");
            Ok(())
        },
        x => {
            print!("{:?}", x);
            Ok(())
        },
    }
}

fn print_depth(depth: u64) {
    for _ in 0..2 * depth {
        print!(" ");
    }
}
