use std::collections::BTreeSet;
use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let values: BTreeSet<u64> = s.split_whitespace().map(|t| t.parse().unwrap()).collect();
    print!("{}", values.iter().rev().nth(1).unwrap());
}
