use std::io::{Read, Write};
fn main() {
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap();
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        let mut run = 1;
        while i + run < data.len() && data[i + run] == b {
            run += 1;
        }
        out.push(b);
        out.extend(run.to_string().bytes());
        i += run;
    }
    std::io::stdout().write_all(&out).unwrap();
}
