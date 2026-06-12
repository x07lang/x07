use std::collections::HashMap;
use std::io::Read;
fn main() {
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap();
    let mut counts: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut word = Vec::new();
    for b in data.iter().chain(std::iter::once(&b' ')) {
        if b.is_ascii_alphabetic() {
            word.push(b.to_ascii_lowercase());
        } else if !word.is_empty() {
            *counts.entry(std::mem::take(&mut word)).or_insert(0) += 1;
        }
    }
    let best = counts.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).unwrap();
    print!("{} {}", String::from_utf8_lossy(best.0), best.1);
}
