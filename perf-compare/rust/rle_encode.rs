use std::io::{Read, Write};

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    if input.is_empty() {
        return;
    }

    let mut output = Vec::with_capacity(input.len() * 2);
    let mut cur = input[0];
    let mut cnt: u8 = 1;

    for &x in &input[1..] {
        if x == cur {
            if cnt < 255 {
                cnt += 1;
            } else {
                output.push(cnt);
                output.push(cur);
                cnt = 1;
            }
        } else {
            output.push(cnt);
            output.push(cur);
            cur = x;
            cnt = 1;
        }
    }

    output.push(cnt);
    output.push(cur);

    std::io::stdout().write_all(&output).unwrap();
}
