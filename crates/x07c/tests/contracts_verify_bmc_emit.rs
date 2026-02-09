use serde_json::json;

use x07c::compile::{compile_program_to_c, CompileOptions, ContractMode};

mod x07_program;

#[test]
fn verify_bmc_contracts_lower_to_cprover_assume_and_assert() {
    let decl = json!({
        "kind": "defn",
        "name": "main.f",
        "params": [{"name": "x", "ty": "i32"}],
        "result": "i32",
        "requires": [{"id":"r0", "expr": ["=", "x", "x"]}],
        "ensures": [{"id":"e0", "expr": ["=", "__result", "x"]}],
        "body": "x"
    });

    let program = x07_program::entry(
        &[],
        vec![decl],
        json!(["begin", ["main.f", 0], ["bytes.alloc", 0]]),
    );
    let options = CompileOptions {
        emit_main: false,
        freestanding: true,
        contract_mode: ContractMode::VerifyBmc,
        ..Default::default()
    };
    let c = compile_program_to_c(program.as_slice(), &options).expect("must compile");

    let assume_count = c.match_indices("__CPROVER_assume(").count();
    let assert_count = c.match_indices("__CPROVER_assert(").count();
    assert!(
        assume_count >= 2,
        "expected at least one __CPROVER_assume call (count={assume_count})"
    );
    assert!(
        assert_count >= 2,
        "expected at least one __CPROVER_assert call (count={assert_count})"
    );

    assert!(
        c.contains("X07T_CONTRACT_V1"),
        "expected contract payload marker in __CPROVER_assert message; first assert: {:?}",
        c.find("__CPROVER_assert(")
            .map(|i| &c[i..std::cmp::min(c.len(), i + 240)])
    );
}
