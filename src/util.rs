pub fn from_be_bytes(bytes: Vec<u8>) -> u64 {
    if bytes.len() > 8 {
        panic!("oops better error handling here");
    }

    let mut register = 0u64;
    for byte in bytes.into_iter() {
        register = (register << 8) | byte as u64;
    }
    register
}
