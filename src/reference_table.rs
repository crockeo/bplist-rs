use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::ops::Index;

use crate::result::Result;
use crate::trailer::Trailer;
use crate::util;

#[derive(Debug)]
pub struct ReferenceTable(HashMap<u64, u64>);

impl ReferenceTable {
    pub fn load(file: &mut File, trailer: &Trailer) -> Result<ReferenceTable> {
        let mut reference_table = ReferenceTable(HashMap::new());

        for i in 0..trailer.num_objects {
            let mut buf = vec![0; trailer.offset_table_offset_size as usize];
            file.read_exact(buf.as_mut_slice())?;
            reference_table.0.insert(i * trailer.offset_table_offset_size as u64, util::from_be_bytes(buf));
        }

        Ok(reference_table)
    }
}

impl Index<&'_ u64> for ReferenceTable {
    type Output = u64;

    fn index(&self, idx: &'_ u64) -> &Self::Output {
        &self.0[&idx]
    }
}

