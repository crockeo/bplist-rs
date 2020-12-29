use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::Read;
use std::ops::Index;

use crate::result::Result;
use crate::trailer::Trailer;
use crate::util;

pub struct ReferenceTable(HashMap<u64, u64>);

impl ReferenceTable {
    pub fn load(file: &mut File, trailer: &Trailer) -> Result<ReferenceTable> {
        let mut reference_table = ReferenceTable(HashMap::new());

        for i in 0..trailer.num_objects {
            let mut buf = vec![0; trailer.offset_table_offset_size as usize];
            file.read_exact(buf.as_mut_slice())?;
            reference_table.0.insert(i, util::from_be_bytes(&buf));
        }

        Ok(reference_table)
    }

    pub fn get<'a>(&self, key: &u64) -> Option<u64> {
        self.0.get(key).map(|idx| *idx)
    }
}

impl Debug for ReferenceTable {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), std::fmt::Error> {
        let mut values = Vec::with_capacity(self.0.len());
        for (key, value) in self.0.iter() {
            values.push((key, value));
        }
        values.sort();

        write!(f, "{{\n")?;
        for (key, value) in values.into_iter() {
            write!(f, "    {}: {}\n", key, value)?;
        }
        write!(f, "}}\n")
    }
}

impl Index<&'_ u64> for ReferenceTable {
    type Output = u64;

    fn index(&self, idx: &'_ u64) -> &Self::Output {
        &self.0[&idx]
    }
}

