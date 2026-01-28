use crate::ast::Expr;

pub(crate) fn stable_fingerprint(expr: &Expr) -> u128 {
    const FNV_OFFSET_BASIS: u128 = 0x6c62272e07bb014262b821756295c58d;
    const FNV_PRIME: u128 = 0x0000000001000000000000000000013b;

    fn write_bytes(mut h: u128, bytes: &[u8]) -> u128 {
        const FNV_PRIME: u128 = 0x0000000001000000000000000000013b;
        for b in bytes {
            h ^= *b as u128;
            h = h.wrapping_mul(FNV_PRIME);
        }
        h
    }

    fn go(mut h: u128, e: &Expr) -> u128 {
        match e {
            Expr::Int { value: i, .. } => {
                h = write_bytes(h, &[0x01]);
                h = write_bytes(h, &i.to_le_bytes());
                h
            }
            Expr::Ident { name: s, .. } => {
                h = write_bytes(h, &[0x02]);
                let len: u32 = s.len() as u32;
                h = write_bytes(h, &len.to_le_bytes());
                h = write_bytes(h, s.as_bytes());
                h
            }
            Expr::List { items, .. } => {
                h = write_bytes(h, &[0x03]);
                let len: u32 = items.len() as u32;
                h = write_bytes(h, &len.to_le_bytes());
                for item in items {
                    h = go(h, item);
                }
                h
            }
        }
    }

    let mut h = FNV_OFFSET_BASIS;
    h = h.wrapping_mul(FNV_PRIME);
    go(h, expr)
}
