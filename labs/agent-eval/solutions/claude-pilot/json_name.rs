use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    // Top-level "name" member; value has no escapes. Scan at depth 1 only.
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' { i += 2; continue; }
            if c == b'"' { in_str = false; }
        } else {
            match c {
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                b'"' => {
                    if depth == 1 && bytes[i..].starts_with(b"\"name\"") {
                        let rest = &s[i + 6..];
                        let colon = rest.find(':').unwrap();
                        let open = rest[colon..].find('"').unwrap() + colon + 1;
                        let close = rest[open..].find('"').unwrap() + open;
                        print!("{}", &rest[open..close]);
                        return;
                    }
                    in_str = true;
                }
                _ => {}
            }
        }
        i += 1;
    }
}
