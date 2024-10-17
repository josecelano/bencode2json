//! Run with:
//!
//! ```not_rust
//! cargo run --example parser_string_in_string_out
//! ```
//!
//! It prints "spam".
use torrust_bencode2json::parsers::BencodeParser;

fn main() {
    let input = "4:spam".to_string();
    let mut output = String::new();

    if let Err(e) = BencodeParser::new(input.as_bytes()).write_str(&mut output) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    println!("{output}");
}
