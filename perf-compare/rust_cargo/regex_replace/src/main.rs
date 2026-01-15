use std::io::{Read, Write};
use regex::Regex;

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    if input.len() < 8 {
        std::io::stdout().write_all(&input).unwrap();
        return;
    }

    let pat_len = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let repl_len = u32::from_le_bytes([input[4], input[5], input[6], input[7]]) as usize;

    if 8 + pat_len + repl_len > input.len() {
        let text_start = 8 + pat_len + repl_len;
        if text_start <= input.len() {
            std::io::stdout().write_all(&input[text_start..]).unwrap();
        }
        return;
    }

    let pattern = match std::str::from_utf8(&input[8..8 + pat_len]) {
        Ok(s) => s,
        Err(_) => {
            std::io::stdout().write_all(&input[8 + pat_len + repl_len..]).unwrap();
            return;
        }
    };

    let replacement = match std::str::from_utf8(&input[8 + pat_len..8 + pat_len + repl_len]) {
        Ok(s) => s,
        Err(_) => {
            std::io::stdout().write_all(&input[8 + pat_len + repl_len..]).unwrap();
            return;
        }
    };

    let text = match std::str::from_utf8(&input[8 + pat_len + repl_len..]) {
        Ok(s) => s,
        Err(_) => {
            std::io::stdout().write_all(&input[8 + pat_len + repl_len..]).unwrap();
            return;
        }
    };

    let result = match Regex::new(pattern) {
        Ok(re) => re.replace_all(text, replacement).into_owned(),
        Err(_) => text.to_string(),
    };

    std::io::stdout().write_all(result.as_bytes()).unwrap();
}
