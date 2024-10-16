//! This lib contains functions to convert bencoded bytes into a JSON string.
//!
//! There are high-level functions for common purposes that call the lower level
//! parser. You can use the low-lever parser if the high-level wrappers are not
//! suitable for your needs.
use parsers::{error::Error, BencodeParser};

pub mod parsers;
pub mod rw;
mod test;

/// It converts bencoded bytes into a JSON string.
///
/// # Errors
///
/// Will return an error if the conversion fails.
pub fn try_bencode_to_json(input_buffer: &[u8]) -> Result<String, Error> {
    let mut output = String::new();

    let mut parser = BencodeParser::new(input_buffer);

    match parser.write_str(&mut output) {
        Ok(()) => Ok(output),
        Err(err) => Err(err),
    }
}

/// Helper to convert a string into a bencoded string.
#[must_use]
pub fn to_bencode(value: &str) -> Vec<u8> {
    let bencoded_str = format!("{}:{}", value.len(), value);
    bencoded_str.as_bytes().to_vec()
}

#[cfg(test)]
mod tests {

    mod converting_bencode_to_json {
        use crate::try_bencode_to_json;

        #[test]
        fn when_it_succeeds() {
            let result = try_bencode_to_json(b"d4:spam4:eggse").unwrap();

            assert_eq!(result, r#"{"spam":"eggs"}"#);
        }

        #[test]
        fn when_it_fails() {
            let result = try_bencode_to_json(b"invalid bencode value");

            assert!(result.is_err());
        }
    }

    mod converting_string_to_bencode {
        use crate::to_bencode;

        #[test]
        fn empty_string() {
            assert_eq!(to_bencode(r""), b"0:");
        }

        #[test]
        fn non_empty_string() {
            assert_eq!(to_bencode(r"alice"), b"5:alice");
        }

        mod string_with_special_chars {
            use crate::to_bencode;

            #[test]
            fn line_break() {
                assert_eq!(to_bencode(r"alice\n"), b"7:alice\x5C\x6E");
            }

            #[test]
            fn utf8_chars() {
                let word = "ñandú";
                assert_eq!(
                    to_bencode(word),
                    format!("{}:{}", word.len(), word).as_bytes()
                );
            }
        }
    }
}
