use std::io::{Read, Write};

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    let acc: u32 = input.iter().map(|&b| b as u32).sum();

    std::io::stdout().write_all(&acc.to_le_bytes()).unwrap();
}
