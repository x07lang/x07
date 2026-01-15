#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let data = if data.len() > 64 * 1024 {
        &data[..64 * 1024]
    } else {
        data
    };

    let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };

    let _ = x07c::ast::expr_from_json(&v);
});
