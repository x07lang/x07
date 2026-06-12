use std::io::Read;
fn main() {
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap();
    print!("{}", data.iter().filter(|b| **b == b'\n').count());
}
