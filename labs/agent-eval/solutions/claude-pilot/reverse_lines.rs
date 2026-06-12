use std::io::{Read, Write};
fn main() {
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap();
    if data.is_empty() { return; }
    let mut lines: Vec<&[u8]> = data.split(|b| *b == b'\n').collect();
    if data.ends_with(b"\n") { lines.pop(); }
    let mut out = Vec::new();
    for line in lines.iter().rev() {
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    std::io::stdout().write_all(&out).unwrap();
}
