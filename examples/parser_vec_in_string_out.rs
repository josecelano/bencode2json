//! Run with:
//!
//! ```not_rust
//! cargo run --example parser_vec_in_string_out
//! ```
//!
//! It prints "spam".
use bencode2json::generators::json::BencodeParser;

fn main() {
    let input = b"4:spam".to_vec();
    let mut output = String::new();

    if let Err(e) = BencodeParser::new(&input[..]).write_str(&mut output) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    println!("{output}");
}
