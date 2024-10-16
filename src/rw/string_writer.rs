//! A writer that writes to an output implementing `std::fmt::Write`.
use core::str;
use std::fmt::Write;

use ringbuffer::{AllocRingBuffer, RingBuffer};

use super::{error::Error, writer::Writer};

/// A writer that writes to an output implementing `std::fmt::Write`.
///
/// It's wrapper of a basic writer with extra functionality.
pub struct StringWriter<W: Write> {
    /// A `std::fmt::Write` writer.
    writer: W,

    /// Number of bytes written to the output.
    output_byte_counter: u64,

    /// The last byte written to the output.
    last_char: Option<char>,

    /// A buffer to capture the latest bytes written to the output.
    captured_chars: AllocRingBuffer<char>,
}

impl<W: Write> StringWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            output_byte_counter: 0,

            last_char: None,
            captured_chars: AllocRingBuffer::new(1024),
        }
    }

    /// Returns the number of bytes that have been written to the output.
    pub fn output_byte_counter(&self) -> u64 {
        self.output_byte_counter
    }

    /// Returns a copy of the bytes that have been written to the output.
    pub fn captured_chars(&self) -> Vec<char> {
        self.captured_chars.to_vec()
    }

    /// Returns the last byte that was written to the output.
    pub fn last_byte(&self) -> Option<char> {
        self.last_char
    }
}

impl<W: Write> Writer for StringWriter<W> {
    fn write_byte(&mut self, byte: u8) -> Result<(), Error> {
        let c = byte as char;

        self.writer.write_char(c)?;

        self.output_byte_counter += 1;

        self.last_char = Some(c);

        self.captured_chars.push(c);

        Ok(())
    }

    fn write_str(&mut self, value: &str) -> Result<(), Error> {
        self.writer.write_str(value)?;

        self.output_byte_counter += value.len() as u64;

        if let Some(last_char) = value.chars().last() {
            self.last_char = Some(last_char);
        }

        for c in value.chars() {
            self.captured_chars.push(c);
        }

        Ok(())
    }

    fn output_byte_counter(&self) -> u64 {
        self.output_byte_counter
    }

    fn captured_bytes(&self) -> Vec<u8> {
        self.captured_chars()
            .into_iter()
            .flat_map(|ch| {
                let mut buf = [0; 4];
                ch.encode_utf8(&mut buf).as_bytes().to_vec()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {

    mod for_writing {
        use crate::rw::{string_writer::StringWriter, writer::Writer};

        #[test]
        fn it_should_write_one_byte_to_the_output() {
            let mut output = String::new();

            let mut string_writer = StringWriter::new(&mut output);

            string_writer.write_byte(b'l').unwrap();

            drop(string_writer);

            assert_eq!(output, "l");
        }

        #[test]
        fn it_should_increase_the_output_byte_counter_by_one_after_writing_a_new_byte() {
            let mut output = String::new();

            let mut string_writer = StringWriter::new(&mut output);

            string_writer.write_byte(b'l').unwrap();

            assert_eq!(string_writer.output_byte_counter(), 1);
        }

        #[test]
        fn it_should_write_strings_to_the_output() {
            let mut output = String::new();

            let mut string_writer = StringWriter::new(&mut output);

            string_writer.write_str("le").unwrap();

            drop(string_writer);

            assert_eq!(output, "le");
        }

        #[test]
        fn it_should_increase_the_output_byte_counter_by_the_string_len_after_writing_a_string() {
            let mut output = String::new();

            let mut string_writer = StringWriter::new(&mut output);

            string_writer.write_str("le").unwrap();

            assert_eq!(string_writer.output_byte_counter(), 2);
        }
    }

    mod for_capturing {

        use crate::rw::{string_writer::StringWriter, writer::Writer};

        #[test]
        fn it_should_return_the_last_written_char() {
            let mut output = String::new();

            let mut string_writer = StringWriter::new(&mut output);

            string_writer.write_byte(b'l').unwrap();

            assert_eq!(string_writer.last_byte(), Some('l'));
        }

        #[test]
        fn it_should_capture_the_latest_written_char() {
            let mut output = String::new();

            let mut string_writer = StringWriter::new(&mut output);

            string_writer.write_byte(b'l').unwrap();

            assert_eq!(string_writer.captured_chars(), vec!['l']);
        }

        #[test]
        fn it_should_capture_1024_chars_at_the_most() {
            let mut output = String::new();

            let mut data = vec!['a'; 1024];
            let latest_104_chars = vec!['b'; 1024];
            data.extend_from_slice(&latest_104_chars);

            let mut string_writer = StringWriter::new(&mut output);

            for c in data {
                string_writer.write_str(&c.to_string()).unwrap();
            }

            assert_eq!(string_writer.captured_chars(), latest_104_chars);
        }
    }
}
