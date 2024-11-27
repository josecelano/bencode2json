//! Run with:
//!
//! ```not_rust
//! cargo run --example try_bencode_to_json
//! ```
use bencode2json::try_bencode_to_json;

fn main() {
    let result = try_bencode_to_json(b"d4:spam4:eggse").unwrap();

    assert_eq!(
        result,
        r#"{"<string>spam</string>":"<string>eggs</string>"}"#
    );
}
