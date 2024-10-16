//! A writer that writes bytes to an output.
//!
//! The output is any type that implements the `std::io::Write` trait.
use core::str;
use std::io::BufWriter;
use std::io::Write;

use ringbuffer::AllocRingBuffer;
use ringbuffer::RingBuffer;

use super::{error::Error, writer::Writer};

/// A writer that writes to an output implementing `std::io::Write`.
///
/// It's wrapper of a basic writer with extra functionality.
pub struct ByteWriter<W: Write> {
    /// It's a buffered writer.
    writer: BufWriter<W>,

    /// Number of bytes written to the output.
    output_byte_counter: u64,

    /// The last byte written to the output.
    last_byte: Option<u8>,

    /// A buffer to capture the latest bytes written to the output.
    captured_bytes: AllocRingBuffer<u8>,
}

impl<W: Write> ByteWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            output_byte_counter: 0,
            writer: BufWriter::new(writer),
            last_byte: None,
            captured_bytes: AllocRingBuffer::new(1024),
        }
    }

    /// Returns the number of bytes that have been written to the output.
    pub fn output_byte_counter(&self) -> u64 {
        self.output_byte_counter
    }

    /// Returns a copy of the bytes that have been written to the output.
    pub fn captured_bytes(&self) -> Vec<u8> {
        self.captured_bytes.to_vec()
    }

    /// Returns the last byte that was written to the output.
    pub fn last_byte(&self) -> Option<u8> {
        self.last_byte
    }
}

impl<W: Write> Writer for ByteWriter<W> {
    fn write_byte(&mut self, byte: u8) -> Result<(), Error> {
        let bytes = [byte];

        self.writer.write_all(&bytes)?;

        self.output_byte_counter += 1;

        self.last_byte = Some(byte);

        self.captured_bytes.push(byte);

        Ok(())
    }

    fn write_str(&mut self, value: &str) -> Result<(), Error> {
        for byte in value.bytes() {
            self.write_byte(byte)?;
        }

        Ok(())
    }

    fn output_byte_counter(&self) -> u64 {
        self.output_byte_counter
    }

    fn captured_bytes(&self) -> Vec<u8> {
        self.captured_bytes()
    }
}

#[cfg(test)]
mod tests {

    mod for_writing {
        use crate::rw::{byte_writer::ByteWriter, writer::Writer};

        #[test]
        fn it_should_write_one_byte_to_the_output() {
            let mut output = Vec::new();

            let mut byte_writer = ByteWriter::new(&mut output);

            byte_writer.write_byte(b'l').unwrap();

            drop(byte_writer);

            assert_eq!(output, vec![b'l']);
        }

        #[test]
        fn it_should_increase_the_output_byte_counter_by_one_after_writing_a_new_byte() {
            let mut output = Vec::new();

            let mut byte_writer = ByteWriter::new(&mut output);

            byte_writer.write_byte(b'l').unwrap();

            assert_eq!(byte_writer.output_byte_counter(), 1);
        }

        #[test]
        fn it_should_write_strings_bytes_to_the_output() {
            let mut output = Vec::new();

            let mut byte_writer = ByteWriter::new(&mut output);

            byte_writer.write_str("l").unwrap();

            drop(byte_writer);

            assert_eq!(output, vec![b'l']);
        }

        #[test]
        fn it_should_increase_the_output_byte_counter_by_the_string_len_after_writing_a_string() {
            let mut output = Vec::new();

            let mut byte_writer = ByteWriter::new(&mut output);

            byte_writer.write_str("le").unwrap();

            assert_eq!(byte_writer.output_byte_counter(), 2);
        }
    }

    mod for_capturing {

        use crate::rw::{byte_writer::ByteWriter, writer::Writer};

        #[test]
        fn it_should_return_the_last_written_byte() {
            let mut output = Vec::new();

            let mut byte_writer = ByteWriter::new(&mut output);

            byte_writer.write_byte(b'l').unwrap();

            assert_eq!(byte_writer.last_byte(), Some(b'l'));
        }

        #[test]
        fn it_should_capture_the_latest_written_bytes() {
            let mut output = Vec::new();

            let mut byte_writer = ByteWriter::new(&mut output);

            byte_writer.write_byte(b'l').unwrap();

            assert_eq!(byte_writer.captured_bytes(), vec![b'l']);
        }

        #[test]
        fn it_should_capture_1024_bytes_at_the_most() {
            let mut output = Vec::new();

            let mut data = vec![b'a'; 1024];
            let last_kilobyte = vec![b'b'; 1024];
            data.extend_from_slice(&last_kilobyte);

            let mut byte_writer = ByteWriter::new(&mut output);

            for byte in data {
                byte_writer.write_byte(byte).unwrap();
            }

            assert_eq!(byte_writer.captured_bytes(), last_kilobyte);
        }
    }
}
