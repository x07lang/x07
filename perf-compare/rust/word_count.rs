use std::io::{Read, Write};

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    let mut cnt: u32 = 0;
    let mut in_word = false;

    for &c in &input {
        let is_space = c == 32 || c == 10 || c == 13 || c == 9;
        if is_space {
            in_word = false;
        } else if !in_word {
            cnt += 1;
            in_word = true;
        }
    }

    std::io::stdout().write_all(&cnt.to_le_bytes()).unwrap();
}
