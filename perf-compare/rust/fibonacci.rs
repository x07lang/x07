use std::io::{Read, Write};

fn main() {
    let mut buf = [0u8; 4];
    std::io::stdin().read_exact(&mut buf).unwrap();
    let n = u32::from_le_bytes(buf);

    let result = if n < 2 {
        n
    } else {
        let mut a: u32 = 0;
        let mut b: u32 = 1;
        for _ in 2..=n {
            let tmp = a.wrapping_add(b);
            a = b;
            b = tmp;
        }
        b
    };

    std::io::stdout().write_all(&result.to_le_bytes()).unwrap();
}
