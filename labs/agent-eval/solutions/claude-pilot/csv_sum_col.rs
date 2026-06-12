use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let total: u64 = s.lines().filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').nth(1).unwrap().parse::<u64>().unwrap()).sum();
    print!("{total}");
}
