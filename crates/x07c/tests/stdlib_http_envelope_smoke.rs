use serde_json::json;
use x07c::compile::{compile_program_to_c_with_meta, CompileOptions};

mod x07_program;

#[test]
fn compile_accepts_std_http_envelope() {
    let program = x07_program::entry(
        &["std.http.envelope"],
        vec![],
        json!([
            "begin",
            [
                "let",
                "req",
                [
                    "bytes.lit",
                    "{\"schema_version\":\"x07.http.request.envelope@0.1.0\",\"id\":\"req1\",\"method\":\"GET\",\"path\":\"/api/ping\",\"headers\":[],\"body\":{\"bytes_len\":0}}\n"
                ]
            ],
            [
                "let",
                "id",
                ["std.http.envelope.extract_id_canon_or_err_v1", ["bytes.view", "req"]]
            ],
            ["let", "headers", ["bytes.lit", "[]"]],
            [
                "std.http.envelope.response_text_canon_v1",
                ["bytes.view", "id"],
                200,
                ["bytes.view", "headers"],
                4,
                ["bytes.view_lit", "\"pong\""]
            ]
        ]),
    );

    compile_program_to_c_with_meta(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}
