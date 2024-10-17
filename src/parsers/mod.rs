//! Parsers, including the main parser and the parsers for the basic types
//! (integer and string)
pub mod error;
pub mod integer;
pub mod stack;
pub mod string;

use std::{
    fmt::Write as FmtWrite,
    io::{self, Read, Write as IoWrite},
};

use derive_more::derive::Display;
use error::{ReadContext, WriteContext};
use stack::{Stack, State};

use crate::rw::{
    byte_reader::ByteReader, byte_writer::ByteWriter, string_writer::StringWriter, writer::Writer,
};

// Bencoded reserved bytes
const BENCODE_BEGIN_INTEGER: u8 = b'i';
const BENCODE_END_INTEGER: u8 = b'e';
const BENCODE_BEGIN_LIST: u8 = b'l';
const BENCODE_BEGIN_DICT: u8 = b'd';
const BENCODE_END_LIST_OR_DICT: u8 = b'e';

#[derive(Debug, PartialEq, Display)]
pub enum BencodeType {
    Integer,
    String,
    List,
    Dict,
}

pub struct BencodeParser<R: Read> {
    byte_reader: ByteReader<R>,
    num_processed_tokens: u64,
    stack: Stack,
}

impl<R: Read> BencodeParser<R> {
    const JSON_ARRAY_BEGIN: u8 = b'[';
    const JSON_ARRAY_ITEMS_SEPARATOR: u8 = b',';
    const JSON_ARRAY_END: u8 = b']';

    const JSON_OBJ_BEGIN: u8 = b'{';
    const JSON_OBJ_FIELDS_SEPARATOR: u8 = b',';
    const JSON_OBJ_FIELD_KEY_VALUE_SEPARATOR: u8 = b':';
    const JSON_OBJ_END: u8 = b'}';

    pub fn new(reader: R) -> Self {
        BencodeParser {
            byte_reader: ByteReader::new(reader),
            num_processed_tokens: 1,
            stack: Stack::default(),
        }
    }

    /// It parses a bencoded value read from input and writes the corresponding
    /// JSON UTF-8 string value to the output.
    ///
    /// # Errors
    ///
    /// Will return an error if it can't read from the input or write to the
    /// output.
    ///
    /// # Panics
    ///
    /// Will panic if receives a byte that isn't a valid begin or end of a
    /// bencoded type: integer, string, list or dictionary.
    pub fn write_str<W: FmtWrite>(&mut self, writer: W) -> Result<(), error::Error> {
        let mut writer = StringWriter::new(writer);
        self.parse(&mut writer)
    }

    /// It parses a bencoded value read from input and writes the corresponding
    /// JSON UTF-8 string value as bytes to the output.
    ///
    /// # Errors
    ///
    /// Will return an error if it can't read from the input or write to the
    /// output.
    ///
    /// # Panics
    ///
    /// Will panic if receives a byte that isn't a valid begin or end of a
    /// bencoded type: integer, string, list or dictionary.
    pub fn write_bytes<W: IoWrite>(&mut self, writer: W) -> Result<(), error::Error> {
        let mut writer = ByteWriter::new(writer);
        self.parse(&mut writer)
    }

    /// It parses a bencoded value read from input and writes the corresponding
    /// JSON value to the output.
    ///
    /// # Errors
    ///
    /// Will return an error if:
    ///
    /// - It can't read from the input or write to the output.
    /// - The input is invalid Bencode.
    fn parse<W: Writer>(&mut self, writer: &mut W) -> Result<(), error::Error> {
        while let Some(peeked_byte) = Self::peek_byte(&mut self.byte_reader, writer)? {
            match peeked_byte {
                BENCODE_BEGIN_INTEGER => {
                    self.begin_bencoded_value(BencodeType::Integer, writer)?;
                    integer::parse(&mut self.byte_reader, writer)?;
                }
                b'0'..=b'9' => {
                    self.begin_bencoded_value(BencodeType::String, writer)?;
                    string::parse(&mut self.byte_reader, writer)?;
                }
                BENCODE_BEGIN_LIST => {
                    let _byte = Self::read_peeked_byte(peeked_byte, &mut self.byte_reader, writer)?;
                    self.begin_bencoded_value(BencodeType::List, writer)?;
                    writer.write_byte(Self::JSON_ARRAY_BEGIN)?;
                    self.stack.push(State::ExpectingFirstListItemOrEnd);
                }
                BENCODE_BEGIN_DICT => {
                    let _byte = Self::read_peeked_byte(peeked_byte, &mut self.byte_reader, writer)?;
                    self.begin_bencoded_value(BencodeType::Dict, writer)?;
                    writer.write_byte(Self::JSON_OBJ_BEGIN)?;
                    self.stack.push(State::ExpectingFirstDictFieldOrEnd);
                }
                BENCODE_END_LIST_OR_DICT => {
                    let _byte = Self::read_peeked_byte(peeked_byte, &mut self.byte_reader, writer)?;
                    self.end_list_or_dict(writer)?;
                }
                b'\n' => {
                    // Ignore line breaks at the beginning, the end, or between values
                    let _byte = Self::read_peeked_byte(peeked_byte, &mut self.byte_reader, writer)?;
                }
                _ => {
                    return Err(error::Error::UnrecognizedFirstBencodeValueByte(
                        ReadContext {
                            byte: Some(peeked_byte),
                            pos: self.byte_reader.input_byte_counter(),
                            latest_bytes: self.byte_reader.captured_bytes(),
                        },
                        WriteContext {
                            byte: Some(peeked_byte),
                            pos: writer.output_byte_counter(),
                            latest_bytes: writer.captured_bytes(),
                        },
                    ));
                }
            }

            self.num_processed_tokens += 1;
        }

        self.check_bad_end_stack_state(writer)
    }

    /// It reads the next byte from the input consuming it. It returns `None` if
    /// the input has ended.
    ///
    /// # Errors
    ///
    /// Will return and errors if:
    ///
    /// - It can't read from the input.
    /// - The byte read is not the expected one (the previously peeked byte).
    fn read_peeked_byte<W: Writer>(
        peeked_byte: u8,
        reader: &mut ByteReader<R>,
        writer: &W,
    ) -> Result<Option<u8>, error::Error> {
        match reader.read_byte() {
            Ok(byte) => {
                if byte == peeked_byte {
                    return Ok(Some(byte));
                }
                Err(error::Error::ReadByteAfterPeekingDoesMatchPeekedByte(
                    ReadContext {
                        byte: Some(byte),
                        pos: reader.input_byte_counter(),
                        latest_bytes: reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: Some(byte),
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ))
            }
            Err(err) => {
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    return Ok(None);
                }
                Err(err.into())
            }
        }
    }

    /// It peeks the next byte from the input without consuming it. It returns
    /// `None` if the input has ended.
    ///
    /// # Errors
    ///
    /// Will return and errors if it can't read from the input.
    fn peek_byte<W: Writer>(
        reader: &mut ByteReader<R>,
        _writer: &W,
    ) -> Result<Option<u8>, error::Error> {
        match reader.peek_byte() {
            Ok(byte) => Ok(Some(byte)),
            Err(err) => {
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    return Ok(None);
                }
                Err(err.into())
            }
        }
    }

    /// It updates the stack state and prints the delimiters when needed.
    ///
    /// Called when the first byt of a bencoded value (integer, string, list or dict)
    /// is received.
    ///
    /// # Errors
    ///
    /// Will return an error if the writer can't write to the output.
    pub fn begin_bencoded_value<W: Writer>(
        &mut self,
        bencode_type: BencodeType,
        writer: &mut W,
    ) -> Result<(), error::Error> {
        match self.stack.peek() {
            State::Initial => {}
            State::ExpectingFirstListItemOrEnd => {
                self.stack.swap_top(State::ExpectingNextListItem);
            }
            State::ExpectingNextListItem => {
                writer.write_byte(Self::JSON_ARRAY_ITEMS_SEPARATOR)?;
            }
            State::ExpectingFirstDictFieldOrEnd => {
                if bencode_type != BencodeType::String {
                    return Err(error::Error::ExpectedStringForDictKeyGot(
                        bencode_type,
                        ReadContext {
                            byte: None,
                            pos: self.byte_reader.input_byte_counter(),
                            latest_bytes: self.byte_reader.captured_bytes(),
                        },
                        WriteContext {
                            byte: None,
                            pos: writer.output_byte_counter(),
                            latest_bytes: writer.captured_bytes(),
                        },
                    ));
                }

                self.stack.swap_top(State::ExpectingDictFieldValue);
            }
            State::ExpectingDictFieldValue => {
                writer.write_byte(Self::JSON_OBJ_FIELD_KEY_VALUE_SEPARATOR)?;

                self.stack.swap_top(State::ExpectingDictFieldKeyOrEnd);
            }
            State::ExpectingDictFieldKeyOrEnd => {
                if bencode_type != BencodeType::String {
                    return Err(error::Error::ExpectedStringForDictKeyGot(
                        bencode_type,
                        ReadContext {
                            byte: None,
                            pos: self.byte_reader.input_byte_counter(),
                            latest_bytes: self.byte_reader.captured_bytes(),
                        },
                        WriteContext {
                            byte: None,
                            pos: writer.output_byte_counter(),
                            latest_bytes: writer.captured_bytes(),
                        },
                    ));
                }

                writer.write_byte(Self::JSON_OBJ_FIELDS_SEPARATOR)?;

                self.stack.swap_top(State::ExpectingDictFieldValue);
            }
        }

        Ok(())
    }

    /// It updates the stack state and prints the delimiters when needed.
    ///
    /// Called when the end of list or dictionary byte is received. End of
    /// integers or strings are processed while parsing them.
    ///
    /// # Errors
    ///
    /// Will return an error if the writer can't write to the output.
    pub fn end_list_or_dict<W: Writer>(&mut self, writer: &mut W) -> Result<(), error::Error> {
        match self.stack.peek() {
            State::ExpectingFirstListItemOrEnd | State::ExpectingNextListItem => {
                writer.write_byte(Self::JSON_ARRAY_END)?;
                self.stack.pop();
            }
            State::ExpectingFirstDictFieldOrEnd | State::ExpectingDictFieldKeyOrEnd => {
                writer.write_byte(Self::JSON_OBJ_END)?;
                self.stack.pop();
            }
            State::ExpectingDictFieldValue => {
                return Err(error::Error::PrematureEndOfDict(
                    ReadContext {
                        byte: None,
                        pos: self.byte_reader.input_byte_counter(),
                        latest_bytes: self.byte_reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: None,
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ))
            }
            State::Initial => {
                return Err(error::Error::NoMatchingStartForListOrDictEnd(
                    ReadContext {
                        byte: None,
                        pos: self.byte_reader.input_byte_counter(),
                        latest_bytes: self.byte_reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: None,
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ))
            }
        }

        Ok(())
    }

    /// It checks if the stack state is correct at the end of the parsing.
    ///
    /// That could happen, for example, when bencode values are not finished.
    ///
    /// # Errors
    ///
    /// Will return an error if the stack state is not correct.
    fn check_bad_end_stack_state<W: Writer>(&self, writer: &W) -> Result<(), error::Error> {
        match self.stack.peek() {
            State::Initial => Ok(()),
            State::ExpectingFirstListItemOrEnd => Err(
                error::Error::UnexpectedEndOfInputExpectingFirstListItemOrEnd(
                    ReadContext {
                        byte: None,
                        pos: self.byte_reader.input_byte_counter(),
                        latest_bytes: self.byte_reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: None,
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ),
            ),
            State::ExpectingNextListItem => {
                Err(error::Error::UnexpectedEndOfInputExpectingNextListItem(
                    ReadContext {
                        byte: None,
                        pos: self.byte_reader.input_byte_counter(),
                        latest_bytes: self.byte_reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: None,
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ))
            }
            State::ExpectingFirstDictFieldOrEnd => Err(
                error::Error::UnexpectedEndOfInputExpectingFirstDictFieldOrEnd(
                    ReadContext {
                        byte: None,
                        pos: self.byte_reader.input_byte_counter(),
                        latest_bytes: self.byte_reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: None,
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ),
            ),
            State::ExpectingDictFieldValue => {
                Err(error::Error::UnexpectedEndOfInputExpectingDictFieldValue(
                    ReadContext {
                        byte: None,
                        pos: self.byte_reader.input_byte_counter(),
                        latest_bytes: self.byte_reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: None,
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ))
            }
            State::ExpectingDictFieldKeyOrEnd => Err(
                error::Error::UnexpectedEndOfInputExpectingDictFieldKeyOrEnd(
                    ReadContext {
                        byte: None,
                        pos: self.byte_reader.input_byte_counter(),
                        latest_bytes: self.byte_reader.captured_bytes(),
                    },
                    WriteContext {
                        byte: None,
                        pos: writer.output_byte_counter(),
                        latest_bytes: writer.captured_bytes(),
                    },
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {

    use std::io::{self, Read};

    use crate::{parsers::BencodeParser, test::bencode_to_json_unchecked, try_bencode_to_json};

    mod it_should_allow_writing {
        use crate::parsers::BencodeParser;

        #[test]
        fn to_any_type_implementing_io_write_trait() {
            let mut output = Vec::new();

            let mut parser = BencodeParser::new(&b"i0e"[..]);

            parser
                .write_bytes(&mut output)
                .expect("Bencode to JSON conversion failed");

            assert_eq!(output, vec!(b'0'));
        }

        #[test]
        fn writing_to_any_type_implementing_fmt_write_trait() {
            let mut output = String::new();

            let mut parser = BencodeParser::new(&b"i0e"[..]);

            parser
                .write_str(&mut output)
                .expect("Bencode to JSON conversion failed");

            assert_eq!(output, "0".to_string());
        }
    }

    #[test]
    fn it_should_allow_reading_from_an_empty_input() {
        struct EmptyReader;

        impl Read for EmptyReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Unexpected EOF",
                ))
            }
        }

        let mut output = String::new();

        let mut parser = BencodeParser::new(EmptyReader);

        parser.write_str(&mut output).unwrap();

        assert_eq!(output, "");
    }

    mod it_should_allow_special_bencode_cases {

        use crate::{parsers::BencodeParser, test::bencode_to_json_unchecked};

        #[test]
        fn an_empty_input() {
            let mut output = String::new();

            let mut parser = BencodeParser::new(&b""[..]);

            parser
                .write_str(&mut output)
                .expect("Bencode to JSON conversion failed");

            assert_eq!(output, String::new());
        }

        #[test]
        fn line_breaks_at_the_beginning_of_the_input_stream() {
            assert_eq!(bencode_to_json_unchecked(b"\ni0e"), "0".to_string());
        }

        #[test]
        fn line_breaks_at_the_end_of_the_input_stream() {
            assert_eq!(bencode_to_json_unchecked(b"i0e\n"), "0".to_string());
        }

        #[test]
        fn line_breaks_between_bencoded_values() {
            assert_eq!(
                bencode_to_json_unchecked(b"li0e\ni1ee"),
                "[0,1]".to_string()
            );
        }
    }

    mod it_should_fail {
        use std::io::{self, Read};

        use crate::{
            parsers::{error::Error, BencodeParser},
            try_bencode_to_json,
        };

        #[test]
        fn when_there_is_a_problem_reading_from_input() {
            struct FaultyReader;

            impl Read for FaultyReader {
                fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Permission denied",
                    ))
                }
            }

            let mut output = String::new();

            let mut parser = BencodeParser::new(FaultyReader);

            let result = parser.write_str(&mut output);

            assert!(matches!(result, Err(Error::Io(_))));
        }

        #[test]
        fn when_it_cannot_recognized_the_fist_byte_of_a_new_bencoded_value() {
            let invalid_bencoded_value = b"a";

            let result = try_bencode_to_json(invalid_bencoded_value);

            assert!(matches!(
                result,
                Err(Error::UnrecognizedFirstBencodeValueByte { .. })
            ));
        }

        #[test]
        fn when_it_reaches_the_end_of_the_input_without_finishing_parsing_a_valid_bencoded_value() {
            let integer_with_missing_end_byte = b"i42";

            let result = try_bencode_to_json(integer_with_missing_end_byte);

            assert!(matches!(
                result,
                Err(Error::UnexpectedEndOfInputParsingInteger { .. })
            ));
        }
    }

    mod integers {
        use crate::test::bencode_to_json_unchecked;

        #[test]
        fn zero() {
            assert_eq!(bencode_to_json_unchecked(b"i0e"), "0".to_string());
        }

        #[test]
        fn one_digit_integer() {
            assert_eq!(bencode_to_json_unchecked(b"i1e"), "1".to_string());
        }

        #[test]
        fn two_digits_integer() {
            assert_eq!(bencode_to_json_unchecked(b"i42e"), "42".to_string());
        }

        #[test]
        fn negative_integer() {
            assert_eq!(bencode_to_json_unchecked(b"i-1e"), "-1".to_string());
        }

        #[test]
        fn positive_integer_greater_than_i64_max() {
            let big_positive_integer = i64::MAX.to_string() + "1";

            let bencoded_big_positive_integer = format!("i{big_positive_integer}e");

            assert_eq!(
                bencode_to_json_unchecked(bencoded_big_positive_integer.as_bytes()),
                big_positive_integer
            );
        }

        #[test]
        fn negative_integer_smaller_than_i64_min() {
            let big_negative_integer = i64::MIN.to_string() + "1";

            let bencoded_big_negative_integer = format!("i{big_negative_integer}e");

            assert_eq!(
                bencode_to_json_unchecked(bencoded_big_negative_integer.as_bytes()),
                big_negative_integer
            );
        }

        mod should_fail {
            use crate::{parsers::error::Error, try_bencode_to_json};

            #[test]
            fn when_it_finds_an_invalid_byte() {
                let int_with_invalid_byte = b"iae";

                let result = try_bencode_to_json(int_with_invalid_byte);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedByteParsingInteger { .. })
                ));
            }

            #[test]
            fn with_duplicate_sign() {
                let int_with_invalid_byte = b"i--42e";

                let result = try_bencode_to_json(int_with_invalid_byte);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedByteParsingInteger { .. })
                ));
            }
        }
    }

    mod strings {
        use crate::{
            test::{bencode_to_json_unchecked, bencoded_string_with_repeated_byte},
            to_bencode,
        };

        #[test]
        fn length_can_contain_leading_zeros() {
            assert_eq!(bencode_to_json_unchecked(b"00:"), r#""""#.to_string());
        }

        #[test]
        fn empty_string() {
            assert_eq!(bencode_to_json_unchecked(b"0:"), r#""""#.to_string());
        }

        #[test]
        fn utf8() {
            assert_eq!(
                bencode_to_json_unchecked(b"4:spam"),
                r#""spam""#.to_string()
            );
        }

        #[test]
        fn non_utf8() {
            assert_eq!(
                bencode_to_json_unchecked(b"4:\xFF\xFE\xFD\xFC"),
                r#""<hex>fffefdfc</hex>""#.to_string()
            );
        }

        #[test]
        fn big_utf8_string() {
            let big_string = "a".repeat(1_000_000);

            assert_eq!(
                bencode_to_json_unchecked(&to_bencode(&big_string)),
                format!(r#""{big_string}""#)
            );
        }

        #[test]
        fn big_non_utf8_string() {
            let big_non_utf8_string = bencoded_string_with_repeated_byte(b'\xFF', 1_000_000);

            let expected = format!(r#""<hex>{}</hex>""#, "ff".repeat(1_000_000));

            assert_eq!(bencode_to_json_unchecked(&big_non_utf8_string), expected);
        }

        #[test]
        fn ending_with_bencode_end_char() {
            assert_eq!(bencode_to_json_unchecked(b"1:e"), r#""e""#.to_string());
        }

        #[test]
        fn containing_a_reserved_char() {
            assert_eq!(bencode_to_json_unchecked(b"1:i"), r#""i""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:l"), r#""l""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:d"), r#""d""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:l"), r#""l""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:e"), r#""e""#.to_string());
        }

        #[test]
        fn containing_a_digit() {
            assert_eq!(bencode_to_json_unchecked(b"1:0"), r#""0""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:1"), r#""1""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:2"), r#""2""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:3"), r#""3""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:4"), r#""4""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:5"), r#""5""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:6"), r#""6""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:7"), r#""7""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:8"), r#""8""#.to_string());
            assert_eq!(bencode_to_json_unchecked(b"1:9"), r#""9""#.to_string());
        }

        mod should_escape_json {
            use crate::{parsers::tests::bencode_to_json_unchecked, to_bencode};

            #[test]
            fn containing_a_double_quote() {
                assert_eq!(
                    bencode_to_json_unchecked("1:\"".as_bytes()),
                    r#""\"""#.to_string()
                );
            }

            #[test]
            fn containing_backslashes() {
                assert_eq!(
                    bencode_to_json_unchecked("1:\\".as_bytes()),
                    r#""\\""#.to_string()
                );
            }

            #[test]
            fn containing_control_characters() {
                assert_eq!(
                    bencode_to_json_unchecked("1:\n".as_bytes()),
                    r#""\n""#.to_string()
                );
                assert_eq!(
                    bencode_to_json_unchecked("1:\r".as_bytes()),
                    r#""\r""#.to_string()
                );
                assert_eq!(
                    bencode_to_json_unchecked("1:\t".as_bytes()),
                    r#""\t""#.to_string()
                );
            }

            #[test]
            fn containing_unicode_characters() {
                assert_eq!(
                    bencode_to_json_unchecked(&to_bencode("ñandú")),
                    r#""ñandú""#.to_string()
                );
            }
        }

        mod it_should_fail_parsing_when {
            use crate::parsers::{error::Error, tests::try_bencode_to_json};

            #[test]
            fn it_reaches_the_end_of_the_input_parsing_the_string_length() {
                let incomplete_string_length = b"4";

                let result = try_bencode_to_json(incomplete_string_length);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedEndOfInputParsingStringLength { .. })
                ));
            }

            #[test]
            fn it_reaches_the_end_of_the_input_parsing_the_string_value() {
                let incomplete_string_value = b"4:123";

                let result = try_bencode_to_json(incomplete_string_value);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedEndOfInputParsingStringValue { .. })
                ));
            }

            #[test]
            fn it_receives_a_non_digit_byte_in_the_string_length() {
                let incomplete_string_value = b"4a:1234";

                let result = try_bencode_to_json(incomplete_string_value);

                assert!(matches!(result, Err(Error::InvalidStringLengthByte { .. })));
            }
        }
    }

    mod lists {
        use crate::{
            parsers::tests::bencode_to_json_unchecked,
            test::{generate_n_nested_empty_bencoded_lists, generate_n_nested_empty_json_arrays},
        };

        #[test]
        fn empty_list() {
            assert_eq!(bencode_to_json_unchecked(b"le"), "[]".to_string());
        }

        #[test]
        fn one_nested_empty_list() {
            assert_eq!(bencode_to_json_unchecked(b"llee"), "[[]]".to_string());
        }

        #[test]
        fn two_nested_empty_list() {
            assert_eq!(bencode_to_json_unchecked(b"llleee"), "[[[]]]".to_string());
        }

        #[test]
        fn many_nested_empty_list() {
            assert_eq!(
                bencode_to_json_unchecked(&generate_n_nested_empty_bencoded_lists(100)),
                generate_n_nested_empty_json_arrays(100)
            );
        }

        mod with_one_item {
            use crate::parsers::tests::bencode_to_json_unchecked;

            #[test]
            fn integer() {
                assert_eq!(bencode_to_json_unchecked(b"li42ee"), "[42]".to_string());
            }

            #[test]
            fn utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l4:spame"),
                    r#"["spam"]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l4:\xFF\xFE\xFD\xFCe"),
                    r#"["<hex>fffefdfc</hex>"]"#.to_string()
                );
            }

            mod of_type_list {
                use crate::parsers::tests::bencode_to_json_unchecked;

                #[test]
                fn two_nested_empty_list() {
                    assert_eq!(bencode_to_json_unchecked(b"llee"), "[[]]".to_string());
                }

                #[test]
                fn three_nested_empty_lists() {
                    assert_eq!(bencode_to_json_unchecked(b"llleee"), "[[[]]]".to_string());
                }

                #[test]
                fn one_nested_list_which_contains_one_integer() {
                    assert_eq!(bencode_to_json_unchecked(b"lli42eee"), "[[42]]".to_string());
                }

                #[test]
                fn one_nested_list_which_contains_two_integers() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"lli42ei43eee"),
                        "[[42,43]]".to_string()
                    );
                }

                #[test]
                fn one_nested_list_which_contains_one_utf_8_string() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ll4:spamee"),
                        r#"[["spam"]]"#.to_string()
                    );
                }

                #[test]
                fn one_nested_list_which_contains_two_utf_8_strings() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ll5:alice3:bobee"),
                        r#"[["alice","bob"]]"#.to_string()
                    );
                }

                #[test]
                fn one_nested_list_which_contains_one_non_utf_8_string() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ll4:\xFF\xFE\xFD\xFCee"),
                        r#"[["<hex>fffefdfc</hex>"]]"#.to_string()
                    );
                }

                #[test]
                fn one_nested_list_which_contains_two_non_utf_8_string() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ll2:\xFF\xFE2:\xFD\xFCee"),
                        r#"[["<hex>fffe</hex>","<hex>fdfc</hex>"]]"#.to_string()
                    );
                }
            }

            mod of_type_dict {
                use crate::parsers::tests::bencode_to_json_unchecked;

                #[test]
                fn empty() {
                    assert_eq!(bencode_to_json_unchecked(b"ldee"), "[{}]".to_string());
                }

                #[test]
                fn with_one_field() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ld3:foo3:baree"),
                        r#"[{"foo":"bar"}]"#.to_string()
                    );
                }

                #[test]
                fn with_two_fields() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ld3:bar4:spam3:fooi42eee"),
                        r#"[{"bar":"spam","foo":42}]"#.to_string()
                    );
                }

                #[test]
                fn with_nested_empty_dict() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ld3:foodeee"),
                        r#"[{"foo":{}}]"#.to_string()
                    );
                }

                #[test]
                fn with_two_nested_empty_dicts() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ld3:food3:foodeeee"),
                        r#"[{"foo":{"foo":{}}}]"#.to_string()
                    );
                }

                #[test]
                fn with_nested_dict_with_one_field() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ld3:food3:foo3:bareee"),
                        r#"[{"foo":{"foo":"bar"}}]"#.to_string()
                    );
                }

                #[test]
                fn with_nested_dict_with_two_fields() {
                    assert_eq!(
                        bencode_to_json_unchecked(b"ld3:food3:foo3:bar3:fooi42eeee"),
                        r#"[{"foo":{"foo":"bar","foo":42}}]"#.to_string()
                    );
                }
            }
        }

        mod with_two_items_of_the_same_type {
            use crate::parsers::tests::bencode_to_json_unchecked;

            #[test]
            fn two_integers() {
                assert_eq!(
                    bencode_to_json_unchecked(b"li42ei43ee"),
                    "[42,43]".to_string()
                );
            }

            #[test]
            fn two_utf8_strings() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l5:alice3:bobe"),
                    r#"["alice","bob"]"#.to_string()
                );
            }

            #[test]
            fn two_non_utf8_strings() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l2:\xFF\xFE2:\xFD\xFCe"),
                    r#"["<hex>fffe</hex>","<hex>fdfc</hex>"]"#.to_string()
                );
            }

            #[test]
            fn two_empty_lists() {
                assert_eq!(bencode_to_json_unchecked(b"llelee"), r"[[],[]]".to_string());
            }

            #[test]
            fn two_empty_dicts() {
                assert_eq!(bencode_to_json_unchecked(b"ldedee"), r"[{},{}]".to_string());
            }

            #[test]
            fn two_lists_with_one_item() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lli42eeli42eee"),
                    r"[[42],[42]]".to_string()
                );
            }

            #[test]
            fn two_dicts_with_one_item() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ld3:fooi42eed3:fooi42eee"),
                    r#"[{"foo":42},{"foo":42}]"#.to_string()
                );
            }
        }

        mod with_two_items_of_different_types {
            use crate::parsers::tests::bencode_to_json_unchecked;

            #[test]
            fn integer_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"li42e5:alicee"),
                    r#"[42,"alice"]"#.to_string()
                );
            }

            #[test]
            fn integer_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"li42e2:\xFF\xFEe"),
                    r#"[42,"<hex>fffe</hex>"]"#.to_string()
                );
            }

            #[test]
            fn integer_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"li42elee"),
                    r"[42,[]]".to_string()
                );
            }

            #[test]
            fn integer_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"li42eli43eee"),
                    r"[42,[43]]".to_string()
                );
            }

            #[test]
            fn integer_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"li42edee"),
                    r"[42,{}]".to_string()
                );
            }

            #[test]
            fn integer_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"li42ed3:fooi42eee"),
                    r#"[42,{"foo":42}]"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l5:alicei42ee"),
                    r#"["alice",42]"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l5:alice1:\xFFe"),
                    r#"["alice","<hex>ff</hex>"]"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l5:alicelee"),
                    r#"["alice",[]]"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l5:aliceli42eee"),
                    r#"["alice",[42]]"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l5:alicedee"),
                    r#"["alice",{}]"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l5:aliced3:fooi42eee"),
                    r#"["alice",{"foo":42}]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l1:\xFFi42ee"),
                    r#"["<hex>ff</hex>",42]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l1:\xFF3:fooe"),
                    r#"["<hex>ff</hex>","foo"]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l1:\xFFlee"),
                    r#"["<hex>ff</hex>",[]]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l1:\xFFli42eee"),
                    r#"["<hex>ff</hex>",[42]]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l1:\xFFdee"),
                    r#"["<hex>ff</hex>",{}]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l1:\xFFd3:fooi42eee"),
                    r#"["<hex>ff</hex>",{"foo":42}]"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"llei42ee"),
                    r"[[],42]".to_string()
                );
            }

            #[test]
            fn empty_list_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lle3:fooe"),
                    r#"[[],"foo"]"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lle1:\xFFe"),
                    r#"[[],"<hex>ff</hex>"]"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_empty_dict() {
                assert_eq!(bencode_to_json_unchecked(b"lledee"), r"[[],{}]".to_string());
            }

            #[test]
            fn empty_list_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lled3:fooi42eee"),
                    r#"[[],{"foo":42}]"#.to_string()
                );
            }

            #[test]
            fn list_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lli42eei43ee"),
                    r"[[42],43]".to_string()
                );
            }

            #[test]
            fn list_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lli42ee3:fooe"),
                    r#"[[42],"foo"]"#.to_string()
                );
            }

            #[test]
            fn list_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lli42ee1:\xFFe"),
                    r#"[[42],"<hex>ff</hex>"]"#.to_string()
                );
            }

            #[test]
            fn list_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lli42eedee"),
                    r"[[42],{}]".to_string()
                );
            }

            #[test]
            fn list_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lli42eed3:fooi43eee"),
                    r#"[[42],{"foo":43}]"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ldei42ee"),
                    r"[{},42]".to_string()
                );
            }

            #[test]
            fn empty_dict_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lde3:fooe"),
                    r#"[{},"foo"]"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"lde1:\xFFe"),
                    r#"[{},"<hex>ff</hex>"]"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_empty_list() {
                assert_eq!(bencode_to_json_unchecked(b"ldelee"), r"[{},[]]".to_string());
            }

            #[test]
            fn empty_dict_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ldeli42eee"),
                    r"[{},[42]]".to_string()
                );
            }

            #[test]
            fn dict_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ld3:fooi42eei43ee"),
                    r#"[{"foo":42},43]"#.to_string()
                );
            }

            #[test]
            fn dict_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ld3:fooi42ee3:fooe"),
                    r#"[{"foo":42},"foo"]"#.to_string()
                );
            }

            #[test]
            fn dict_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ld3:fooi42ee1:\xFFe"),
                    r#"[{"foo":42},"<hex>ff</hex>"]"#.to_string()
                );
            }

            #[test]
            fn dict_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ld3:fooi42eelee"),
                    r#"[{"foo":42},[]]"#.to_string()
                );
            }

            #[test]
            fn dict_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"ld3:fooi42eeli43eee"),
                    r#"[{"foo":42},[43]]"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_an_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"l2:\xFF\xFEi42ee"),
                    r#"["<hex>fffe</hex>",42]"#.to_string()
                );
            }
        }

        mod should_fail {
            use crate::{parsers::error::Error, try_bencode_to_json};

            #[test]
            fn when_an_empty_list_does_not_have_the_matching_close_byte() {
                let list_without_closing_list_byte = b"l";

                let result = try_bencode_to_json(list_without_closing_list_byte);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedEndOfInputExpectingFirstListItemOrEnd { .. })
                ));
            }

            #[test]
            fn when_a_list_does_not_have_the_matching_close_byte() {
                let list_without_closing_list_byte = b"li42e";

                let result = try_bencode_to_json(list_without_closing_list_byte);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedEndOfInputExpectingNextListItem { .. })
                ));
            }

            #[test]
            fn when_it_receives_an_end_list_byte_without_the_matching_open_byte() {
                let end_list_byte_without_start = b"e";

                let result = try_bencode_to_json(end_list_byte_without_start);

                assert!(matches!(
                    result,
                    Err(Error::NoMatchingStartForListOrDictEnd { .. })
                ));
            }
        }
    }

    mod dictionary {
        use crate::{
            parsers::tests::bencode_to_json_unchecked,
            test::{
                generate_n_nested_empty_bencoded_dictionaries, generate_n_nested_empty_json_objects,
            },
        };

        #[test]
        fn empty_dictionary() {
            assert_eq!(bencode_to_json_unchecked(b"de"), "{}".to_string());
        }

        #[test]
        fn one_nested_empty_dictionary() {
            assert_eq!(
                bencode_to_json_unchecked(b"d3:foodee"),
                r#"{"foo":{}}"#.to_string()
            );
        }

        #[test]
        fn two_nested_empty_dictionaries() {
            assert_eq!(
                bencode_to_json_unchecked(b"d3:food3:foodeee"),
                r#"{"foo":{"foo":{}}}"#.to_string()
            );
        }

        #[test]
        fn many_nested_empty_dictionaries() {
            assert_eq!(
                bencode_to_json_unchecked(&generate_n_nested_empty_bencoded_dictionaries(100)),
                generate_n_nested_empty_json_objects(100)
            );
        }

        mod with_a_key {
            use crate::parsers::tests::bencode_to_json_unchecked;

            #[test]
            fn starting_with_a_digit() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d4:1fooi42ee"),
                    r#"{"1foo":42}"#.to_string()
                );
            }

            #[test]
            fn which_is_not_a_utf_8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d2:\xFF\xFEi42ee"),
                    r#"{"<hex>fffe</hex>":42}"#.to_string()
                );
            }
        }

        mod with_one_field {
            use crate::parsers::tests::bencode_to_json_unchecked;

            #[test]
            fn integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:fooi42ee"),
                    r#"{"foo":42}"#.to_string()
                );
            }

            #[test]
            fn utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar4:spame"),
                    r#"{"bar":"spam"}"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar2:\xFF\xFEe"),
                    r#"{"bar":"<hex>fffe</hex>"}"#.to_string()
                );
            }

            #[test]
            fn empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barlee"),
                    r#"{"bar":[]}"#.to_string()
                );
            }

            #[test]
            fn empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bardee"),
                    r#"{"bar":{}}"#.to_string()
                );
            }
        }

        mod with_two_fields_of_the_same_type {
            use crate::parsers::tests::bencode_to_json_unchecked;

            #[test]
            fn two_integers() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bari42e3:fooi43ee"),
                    r#"{"bar":42,"foo":43}"#.to_string()
                );
            }

            #[test]
            fn two_empty_utf8_strings() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar0:3:foo0:e"),
                    r#"{"bar":"","foo":""}"#.to_string()
                );
            }

            #[test]
            fn two_utf8_strings() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar4:spam3:foo5:alicee"),
                    r#"{"bar":"spam","foo":"alice"}"#.to_string()
                );
            }

            #[test]
            fn two_non_utf8_strings() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar1:\xFF3:foo1:\xFEe"),
                    r#"{"bar":"<hex>ff</hex>","foo":"<hex>fe</hex>"}"#.to_string()
                );
            }

            #[test]
            fn two_empty_lists() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barle3:foolee"),
                    r#"{"bar":[],"foo":[]}"#.to_string()
                );
            }

            #[test]
            fn two_empty_dicts() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barde3:foodee"),
                    r#"{"bar":{},"foo":{}}"#.to_string()
                );
            }

            #[test]
            fn two_lists() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barli42ee3:fooli43eee"),
                    r#"{"bar":[42],"foo":[43]}"#.to_string()
                );
            }

            #[test]
            fn two_dicts() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bard3:bardee3:food3:foodeee"),
                    r#"{"bar":{"bar":{}},"foo":{"foo":{}}}"#.to_string()
                );
            }
        }

        mod with_two_fields_of_different_type {
            use crate::test::bencode_to_json_unchecked;

            #[test]
            fn integer_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bari42e3:foo5:alicee"),
                    r#"{"bar":42,"foo":"alice"}"#.to_string()
                );
            }

            #[test]
            fn integer_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bari42e3:foo1:\xFFe"),
                    r#"{"bar":42,"foo":"<hex>ff</hex>"}"#.to_string()
                );
            }

            #[test]
            fn integer_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bari42e3:foolee"),
                    r#"{"bar":42,"foo":[]}"#.to_string()
                );
            }

            #[test]
            fn integer_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bari42e3:fooli43eee"),
                    r#"{"bar":42,"foo":[43]}"#.to_string()
                );
            }

            #[test]
            fn integer_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bari42e3:foodee"),
                    r#"{"bar":42,"foo":{}}"#.to_string()
                );
            }

            #[test]
            fn integer_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bari42e3:food3:fooi43eee"),
                    r#"{"bar":42,"foo":{"foo":43}}"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar5:alice3:fooi43ee"),
                    r#"{"bar":"alice","foo":43}"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar5:alice3:foo1:\xFFe"),
                    r#"{"bar":"alice","foo":"<hex>ff</hex>"}"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar5:alice3:foolee"),
                    r#"{"bar":"alice","foo":[]}"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar5:alice3:fooli42eee"),
                    r#"{"bar":"alice","foo":[42]}"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar5:alice3:foodee"),
                    r#"{"bar":"alice","foo":{}}"#.to_string()
                );
            }

            #[test]
            fn utf8_string_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar5:alice3:food3:fooi42eee"),
                    r#"{"bar":"alice","foo":{"foo":42}}"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar1:\xFF3:fooi43ee"),
                    r#"{"bar":"<hex>ff</hex>","foo":43}"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar1:\xFF3:foo1:\xFFe"),
                    r#"{"bar":"<hex>ff</hex>","foo":"<hex>ff</hex>"}"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar1:\xFF3:foolee"),
                    r#"{"bar":"<hex>ff</hex>","foo":[]}"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar1:\xFF3:fooli42eee"),
                    r#"{"bar":"<hex>ff</hex>","foo":[42]}"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar1:\xFF3:foodee"),
                    r#"{"bar":"<hex>ff</hex>","foo":{}}"#.to_string()
                );
            }

            #[test]
            fn non_utf8_string_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bar1:\xFF3:food3:fooi42eee"),
                    r#"{"bar":"<hex>ff</hex>","foo":{"foo":42}}"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barle3:fooi42ee"),
                    r#"{"bar":[],"foo":42}"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barle3:foo5:alicee"),
                    r#"{"bar":[],"foo":"alice"}"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barle3:foo1:\xFFe"),
                    r#"{"bar":[],"foo":"<hex>ff</hex>"}"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barle3:foodee"),
                    r#"{"bar":[],"foo":{}}"#.to_string()
                );
            }

            #[test]
            fn empty_list_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barle3:food3:foo5:aliceee"),
                    r#"{"bar":[],"foo":{"foo":"alice"}}"#.to_string()
                );
            }

            #[test]
            fn list_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barli42ee3:fooi42ee"),
                    r#"{"bar":[42],"foo":42}"#.to_string()
                );
            }

            #[test]
            fn list_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barli42ee3:foo5:alicee"),
                    r#"{"bar":[42],"foo":"alice"}"#.to_string()
                );
            }

            #[test]
            fn list_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barli42ee3:foo1:\xFFe"),
                    r#"{"bar":[42],"foo":"<hex>ff</hex>"}"#.to_string()
                );
            }

            #[test]
            fn list_and_empty_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barli42ee3:foodee"),
                    r#"{"bar":[42],"foo":{}}"#.to_string()
                );
            }

            #[test]
            fn list_and_dict() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barli42ee3:food3:foo5:aliceee"),
                    r#"{"bar":[42],"foo":{"foo":"alice"}}"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barde3:fooi42ee"),
                    r#"{"bar":{},"foo":42}"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barde3:foo5:alicee"),
                    r#"{"bar":{},"foo":"alice"}"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barde3:foo1:\xFFe"),
                    r#"{"bar":{},"foo":"<hex>ff</hex>"}"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barde3:foolee"),
                    r#"{"bar":{},"foo":[]}"#.to_string()
                );
            }

            #[test]
            fn empty_dict_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:barde3:fooli42eee"),
                    r#"{"bar":{},"foo":[42]}"#.to_string()
                );
            }

            #[test]
            fn dict_and_integer() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bard3:bari42ee3:fooi43ee"),
                    r#"{"bar":{"bar":42},"foo":43}"#.to_string()
                );
            }

            #[test]
            fn dict_and_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bard3:bari42ee3:foo5:alicee"),
                    r#"{"bar":{"bar":42},"foo":"alice"}"#.to_string()
                );
            }

            #[test]
            fn dict_and_non_utf8_string() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bard3:bari42ee3:foo1:\xFFe"),
                    r#"{"bar":{"bar":42},"foo":"<hex>ff</hex>"}"#.to_string()
                );
            }

            #[test]
            fn dict_and_empty_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bard3:bari42ee3:foolee"),
                    r#"{"bar":{"bar":42},"foo":[]}"#.to_string()
                );
            }

            #[test]
            fn dict_and_list() {
                assert_eq!(
                    bencode_to_json_unchecked(b"d3:bard3:bari42ee3:fooli42eee"),
                    r#"{"bar":{"bar":42},"foo":[42]}"#.to_string()
                );
            }
        }

        mod should_escape_json {

            mod in_field_keys {
                use crate::parsers::tests::bencode_to_json_unchecked;

                // Only one especial char is tested. The string parser contains
                // other tests for the rest of the special chars that need to be
                // escaped.

                #[test]
                fn containing_a_line_break_at_the_beginning_of_the_string() {
                    assert_eq!(
                        bencode_to_json_unchecked("d4:\nfoo3:bare".as_bytes()),
                        r#"{"\nfoo":"bar"}"#.to_string()
                    );
                }

                #[test]
                fn containing_a_line_break_in_the_middle_of_the_string() {
                    assert_eq!(
                        bencode_to_json_unchecked("d4:f\noo3:bare".as_bytes()),
                        r#"{"f\noo":"bar"}"#.to_string()
                    );
                }

                #[test]
                fn containing_a_line_break_at_the_end_of_the_string() {
                    assert_eq!(
                        bencode_to_json_unchecked("d4:foo\n3:bare".as_bytes()),
                        r#"{"foo\n":"bar"}"#.to_string()
                    );
                }
            }

            mod in_field_values {
                use crate::parsers::tests::bencode_to_json_unchecked;

                #[test]
                fn containing_a_line_break_at_the_beginning_of_the_string() {
                    assert_eq!(
                        bencode_to_json_unchecked("d3:foo4:\nbare".as_bytes()),
                        r#"{"foo":"\nbar"}"#.to_string()
                    );
                }

                #[test]
                fn containing_a_line_break_in_the_middle_of_the_string() {
                    assert_eq!(
                        bencode_to_json_unchecked("d3:foo4:ba\nre".as_bytes()),
                        r#"{"foo":"ba\nr"}"#.to_string()
                    );
                }

                #[test]
                fn containing_a_line_break_at_the_end_of_the_string() {
                    assert_eq!(
                        bencode_to_json_unchecked("d3:foo4:bar\ne".as_bytes()),
                        r#"{"foo":"bar\n"}"#.to_string()
                    );
                }
            }
        }

        mod should_fail {
            use crate::{parsers::error::Error, try_bencode_to_json};

            #[test]
            fn when_an_empty_dict_does_not_have_the_matching_close_byte() {
                let dict_without_closing_dict_byte = b"d";

                let result = try_bencode_to_json(dict_without_closing_dict_byte);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedEndOfInputExpectingFirstDictFieldOrEnd { .. })
                ));
            }

            #[test]
            fn when_a_dict_field_does_not_have_the_value() {
                let dict_without_closing_dict_byte = b"d3:foo";

                let result = try_bencode_to_json(dict_without_closing_dict_byte);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedEndOfInputExpectingDictFieldValue { .. })
                ));
            }

            #[test]
            fn when_a_dict_does_not_have_the_matching_close_byte() {
                let dict_without_closing_dict_byte = b"d3:fooi42e";

                let result = try_bencode_to_json(dict_without_closing_dict_byte);

                assert!(matches!(
                    result,
                    Err(Error::UnexpectedEndOfInputExpectingDictFieldKeyOrEnd { .. })
                ));
            }

            #[test]
            fn when_it_receives_an_end_dict_byte_without_the_matching_open_byte() {
                let end_dict_byte_without_start = b"e";

                let result = try_bencode_to_json(end_dict_byte_without_start);

                assert!(matches!(
                    result,
                    Err(Error::NoMatchingStartForListOrDictEnd { .. })
                ));
            }

            #[test]
            fn when_it_receives_a_premature_end_dict_byte() {
                let dict_with_missing_key_value = b"d3:fooe";

                let result = try_bencode_to_json(dict_with_missing_key_value);

                assert!(matches!(result, Err(Error::PrematureEndOfDict { .. })));
            }

            #[test]
            fn when_the_first_field_value_is_empty() {
                let dict_with_missing_key_value = b"d3:fooe";

                let result = try_bencode_to_json(dict_with_missing_key_value);

                assert!(matches!(result, Err(Error::PrematureEndOfDict { .. })));
            }

            #[test]
            fn when_the_second_field_value_is_empty() {
                let dict_with_missing_key_value = b"d3:foo3:bar3:fooe";

                let result = try_bencode_to_json(dict_with_missing_key_value);

                assert!(matches!(result, Err(Error::PrematureEndOfDict { .. })));
            }

            mod when_the_field_key_is_not_a_string_for_example {
                use crate::parsers::error::Error;
                use crate::parsers::BencodeType;
                use crate::try_bencode_to_json;

                #[test]
                fn when_the_key_in_the_first_dict_field_is_an_integer() {
                    let field_with_integer_key = b"di42ei43ee";

                    let result = try_bencode_to_json(field_with_integer_key);

                    assert!(matches!(
                        result,
                        Err(Error::ExpectedStringForDictKeyGot(
                            BencodeType::Integer,
                            _,
                            _
                        ))
                    ));
                }

                #[test]
                fn when_the_key_in_the_second_dict_field_is_an_integer() {
                    let field_with_integer_key = b"d3:foo3:bari42ei43ee";

                    let result = try_bencode_to_json(field_with_integer_key);

                    assert!(matches!(
                        result,
                        Err(Error::ExpectedStringForDictKeyGot(
                            BencodeType::Integer,
                            _,
                            _
                        ))
                    ));
                }

                #[test]
                fn when_the_key_in_the_first_dict_field_is_a_list() {
                    let field_with_list_key = b"dlei42ee";

                    let result = try_bencode_to_json(field_with_list_key);

                    assert!(matches!(
                        result,
                        Err(Error::ExpectedStringForDictKeyGot(BencodeType::List, _, _))
                    ));
                }

                #[test]
                fn when_the_key_in_the_second_dict_field_is_a_list() {
                    let field_with_list_key = b"d3:foo3:barlei42ee";

                    let result = try_bencode_to_json(field_with_list_key);

                    assert!(matches!(
                        result,
                        Err(Error::ExpectedStringForDictKeyGot(BencodeType::List, _, _))
                    ));
                }

                #[test]
                fn when_the_key_in_the_first_dict_field_is_a_dict() {
                    let field_with_list_key = b"ddei42ee";

                    let result = try_bencode_to_json(field_with_list_key);

                    assert!(matches!(
                        result,
                        Err(Error::ExpectedStringForDictKeyGot(BencodeType::Dict, _, _))
                    ));
                }

                #[test]
                fn when_the_key_in_the_second_dict_field_is_a_dict() {
                    let field_with_list_key = b"d3:foo3:bardei42ee";

                    let result = try_bencode_to_json(field_with_list_key);

                    assert!(matches!(
                        result,
                        Err(Error::ExpectedStringForDictKeyGot(BencodeType::Dict, _, _))
                    ));
                }
            }
        }
    }
}
