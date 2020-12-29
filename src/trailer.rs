use std::fs::File;
use std::io::Read;

use crate::result::Result;

pub struct Trailer {
    pub offset_table_offset_size: u8,
    pub object_ref_size: u8,
    pub num_objects: u64,
    pub top_object_offset: u64,
    pub offset_table_start: u64,
}

impl Trailer {
    pub fn load(file: &mut File) -> Result<Trailer> {
        let mut buf = [0; 8];
        file.read_exact(&mut buf[0..6])?;

        file.read_exact(&mut buf[0..1])?;
        let offset_table_offset_size = buf[0..1][0];

        file.read_exact(&mut buf[0..1])?;
        let object_ref_size = buf[0..1][0];

        file.read_exact(&mut buf)?;
        let num_objects = u64::from_be_bytes(buf);

        file.read_exact(&mut buf)?;
        let top_object_offset = u64::from_be_bytes(buf);

        file.read_exact(&mut buf)?;
        let offset_table_start = u64::from_be_bytes(buf);

        Ok(Trailer {
            offset_table_offset_size,
            object_ref_size,
            num_objects,
            top_object_offset,
            offset_table_start,
        })
    }
}
