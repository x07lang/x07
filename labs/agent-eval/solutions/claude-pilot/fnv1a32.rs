use std::io::Read;
fn main() {
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap();
    let mut h: u32 = 2166136261;
    for b in data {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    print!("{h}");
}
