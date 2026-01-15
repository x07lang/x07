use std::io::{Read, Write};
use regex::Regex;

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    if input.len() < 4 {
        let result: u32 = 0;
        std::io::stdout().write_all(&result.to_le_bytes()).unwrap();
        return;
    }

    let pat_len = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;

    if 4 + pat_len > input.len() {
        let result: u32 = 0;
        std::io::stdout().write_all(&result.to_le_bytes()).unwrap();
        return;
    }

    let pattern = match std::str::from_utf8(&input[4..4 + pat_len]) {
        Ok(s) => s,
        Err(_) => {
            let result: u32 = 0;
            std::io::stdout().write_all(&result.to_le_bytes()).unwrap();
            return;
        }
    };

    let text = match std::str::from_utf8(&input[4 + pat_len..]) {
        Ok(s) => s,
        Err(_) => {
            let result: u32 = 0;
            std::io::stdout().write_all(&result.to_le_bytes()).unwrap();
            return;
        }
    };

    let result: u32 = match Regex::new(pattern) {
        Ok(re) => if re.is_match(text) { 1 } else { 0 },
        Err(_) => 0,
    };

    std::io::stdout().write_all(&result.to_le_bytes()).unwrap();
}
