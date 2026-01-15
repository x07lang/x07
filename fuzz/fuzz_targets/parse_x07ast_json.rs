#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let data = if data.len() > 64 * 1024 {
        &data[..64 * 1024]
    } else {
        data
    };

    let _ = x07c::x07ast::parse_x07ast_json(data);
});
