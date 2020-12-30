use std::fs::File;

mod bplist;
mod imessage;

fn main() -> bplist::Result<()> {
    let mut file = File::open("test.ichat")?;
    let bplist = bplist::BPList::load(&mut file)?;

    imessage::explore(bplist)?;

    Ok(())
}
