//! Bencode tokenizer. Given an input stream, it returns a stream of tokens.
use std::io::{self, Read};

use super::{
    error::{self, ReadContext, WriteContext},
    integer, string,
};

use crate::rw::{byte_reader::ByteReader, byte_writer::ByteWriter, writer::Writer};

/* TODO:

- Remove writer from tokenizer.
- Implement trait Iterator for tokenizer.

*/

// Bencoded reserved bytes
const BENCODE_BEGIN_INTEGER: u8 = b'i';
pub const BENCODE_END_INTEGER: u8 = b'e';
const BENCODE_BEGIN_LIST: u8 = b'l';
const BENCODE_BEGIN_DICT: u8 = b'd';
const BENCODE_END_LIST_OR_DICT: u8 = b'e';

#[derive(Debug, PartialEq)]
pub enum BencodeToken {
    Integer(Vec<u8>),
    String(Vec<u8>),
    BeginList,
    BeginDict,
    EndListOrDict,
    LineBreak,
}

pub struct BencodeTokenizer<R: Read> {
    byte_reader: ByteReader<R>,
}

impl<R: Read> BencodeTokenizer<R> {
    pub fn new(reader: R) -> Self {
        BencodeTokenizer {
            byte_reader: ByteReader::new(reader),
        }
    }

    /// It parses the next bencoded token from input.
    ///
    /// # Errors
    ///
    /// Will return an error if:
    ///
    /// - It can't read from the input.
    pub fn next_token<W: Writer>(
        &mut self,
        writer: &mut W,
    ) -> Result<Option<BencodeToken>, error::Error> {
        let capture_output = Vec::new();
        let mut null_writer = ByteWriter::new(capture_output);

        let opt_peeked_byte = Self::peek_byte(&mut self.byte_reader, &null_writer)?;

        match opt_peeked_byte {
            Some(peeked_byte) => {
                match peeked_byte {
                    BENCODE_BEGIN_INTEGER => {
                        let value = integer::parse(&mut self.byte_reader, &mut null_writer)?;
                        Ok(Some(BencodeToken::Integer(value)))
                    }
                    b'0'..=b'9' => {
                        let value = string::parse(&mut self.byte_reader, &mut null_writer)?;
                        Ok(Some(BencodeToken::String(value)))
                    }
                    BENCODE_BEGIN_LIST => {
                        let _byte = Self::read_peeked_byte(
                            peeked_byte,
                            &mut self.byte_reader,
                            &null_writer,
                        )?;
                        Ok(Some(BencodeToken::BeginList))
                    }
                    BENCODE_BEGIN_DICT => {
                        let _byte = Self::read_peeked_byte(
                            peeked_byte,
                            &mut self.byte_reader,
                            &null_writer,
                        )?;
                        Ok(Some(BencodeToken::BeginDict))
                    }
                    BENCODE_END_LIST_OR_DICT => {
                        let _byte = Self::read_peeked_byte(
                            peeked_byte,
                            &mut self.byte_reader,
                            &null_writer,
                        )?;
                        Ok(Some(BencodeToken::EndListOrDict))
                    }
                    b'\n' => {
                        // todo: we should not return any token and continue to the next token.
                        // Ignore line breaks at the beginning, the end, or between values
                        let _byte = Self::read_peeked_byte(
                            peeked_byte,
                            &mut self.byte_reader,
                            &null_writer,
                        )?;
                        Ok(Some(BencodeToken::LineBreak))
                    }
                    _ => Err(error::Error::UnrecognizedFirstBencodeValueByte(
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
                    )),
                }
            }
            None => Ok(None),
        }
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

    /// Returns the number of bytes that have been read from the input.
    pub fn input_byte_counter(&self) -> u64 {
        self.byte_reader.input_byte_counter()
    }

    /// Returns a copy of the bytes that have been read from the input.
    pub fn captured_bytes(&self) -> Vec<u8> {
        self.byte_reader.captured_bytes()
    }
}
