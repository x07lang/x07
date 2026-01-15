use std::io::{Read, Write};

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    let mut freq = [0u32; 256];

    for &b in &input {
        freq[b as usize] += 1;
    }

    let mut output = Vec::with_capacity(256 * 5);

    for (j, &count) in freq.iter().enumerate() {
        if count > 0 {
            output.push(j as u8);
            output.extend_from_slice(&count.to_le_bytes());
        }
    }

    std::io::stdout().write_all(&output).unwrap();
}
