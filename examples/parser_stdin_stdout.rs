//! Run with:
//!
//! ```not_rust
//! echo "4:spam" | cargo run --example parser_stdin_stdout
//! ```
//!
//! It prints "spam".
use std::io;

use torrust_bencode2json::parsers::BencodeParser;

fn main() {
    let input = Box::new(io::stdin());
    let mut output = Box::new(io::stdout());

    if let Err(e) = BencodeParser::new(input).write_bytes(&mut output) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
