use std::io::{Read, Write};
fn main() {
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap();
    let out: Vec<u8> = data.iter().map(|b| b.to_ascii_uppercase()).collect();
    std::io::stdout().write_all(&out).unwrap();
}
