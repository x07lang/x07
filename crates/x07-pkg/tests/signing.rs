use base64::Engine as _;
use ed25519_dalek::Signer as _;

#[test]
fn pkg_sig_message_v1_is_stable() {
    let msg = x07_pkg::pkg_sig_message_v1(
        "hello",
        "0.1.0",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_eq!(
        msg,
        b"x07.pkg.sig.v1\nname=hello\nversion=0.1.0\nsha256=0000000000000000000000000000000000000000000000000000000000000000\n"
    );
}

#[test]
fn verify_ed25519_signature_b64_accepts_valid_signature() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let verifying_key = signing_key.verifying_key();

    let msg = x07_pkg::pkg_sig_message_v1(
        "integrity-demo",
        "0.1.0",
        "6e9d09575894ac86dbb79b187ceba38a4a0bacce113120dce6037c4ca92ec022",
    );
    let sig = signing_key.sign(&msg);

    let b64 = base64::engine::general_purpose::STANDARD;
    let pub_b64 = b64.encode(verifying_key.to_bytes());
    let sig_b64 = b64.encode(sig.to_bytes());

    x07_pkg::verify_ed25519_signature_b64(&pub_b64, &msg, &sig_b64).expect("verify signature");
}
