use super::bplist::{BPList, Result};

pub fn explore(bplist: BPList) -> Result<()> {
    // prints out all of the text messages
    if let BPList::Array(items) = bplist.gets("$objects")? {
        for item in items.into_iter() {
            let class = match item.gets("$class") {
                Err(_) => continue,
                Ok(class) => class,
            };

            if class == &BPList::UID(vec![18]) {
                println!("{:?}", item.gets("NS.string")?);
            }
        }
    }

    Ok(())
}
