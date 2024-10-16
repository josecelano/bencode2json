//! A reader that reads bytes from an input.
//!
//! The input is any type that implements the `std::io::Read` trait.
use std::io::BufReader;
use std::io::Error;
use std::io::Read;

use ringbuffer::AllocRingBuffer;
use ringbuffer::RingBuffer;

/// A reader that reads bytes from an input.
///
/// It's wrapper of a basic reader with extra functionality.
pub struct ByteReader<R: Read> {
    /// It's a buffered reader.
    reader: BufReader<R>,

    /// Number of bytes read from the input.
    input_byte_counter: u64,

    /// The peeked byte when we peek instead or reading.
    peeked_byte: Option<u8>,

    /// The last byte read from the input.
    last_byte: Option<u8>,

    /// A buffer to capture the latest bytes read from the input.
    captured_bytes: AllocRingBuffer<u8>,
}

impl<R: Read> ByteReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            input_byte_counter: 0,
            peeked_byte: None,
            last_byte: None,
            captured_bytes: AllocRingBuffer::new(1024),
        }
    }

    /// It reads one byte from the input.
    ///
    /// # Errors
    ///
    /// Will return an error if it can't read the byte from the input.
    pub fn read_byte(&mut self) -> Result<u8, Error> {
        if let Some(byte) = self.peeked_byte.take() {
            return Ok(byte);
        }

        let mut byte = [0; 1];

        self.reader.read_exact(&mut byte)?;

        self.input_byte_counter += 1;

        let byte = byte[0];

        self.last_byte = Some(byte);
        self.captured_bytes.push(byte);

        Ok(byte)
    }

    /// Peeks at the next byte in the input without consuming it.
    ///
    /// # Errors
    ///
    /// Will return an error if it can't read the byte from the input.
    pub fn peek_byte(&mut self) -> Result<u8, Error> {
        let byte = if let Some(byte) = self.peeked_byte {
            byte
        } else {
            let byte = self.read_byte()?;
            self.peeked_byte = Some(byte);
            byte
        };

        Ok(byte)
    }

    /// Returns the number of bytes that have been read from the input.
    pub fn input_byte_counter(&self) -> u64 {
        self.input_byte_counter
    }

    /// Returns a copy of the bytes that have been read from the input.
    pub fn captured_bytes(&self) -> Vec<u8> {
        self.captured_bytes.to_vec()
    }

    /// Returns the last byte that was read from the input.
    pub fn last_byte(&self) -> Option<u8> {
        self.last_byte
    }
}

#[cfg(test)]
mod tests {

    mod for_reading {
        use crate::rw::byte_reader::ByteReader;

        #[test]
        fn it_should_read_one_byte_from_the_input_consuming_it() {
            let input = vec![b'l', b'e'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            assert_eq!(byte_reader.read_byte().unwrap(), b'l');
            assert_eq!(byte_reader.read_byte().unwrap(), b'e');
        }

        #[test]
        fn it_should_fail_when_there_are_no_more_bytes_to_read() {
            let input = vec![b'l', b'e'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            assert_eq!(byte_reader.read_byte().unwrap(), b'l');
            assert_eq!(byte_reader.read_byte().unwrap(), b'e');
            assert!(byte_reader.read_byte().is_err());
        }

        #[test]
        fn it_should_increase_the_input_byte_counter_by_one_when_reading_a_new_byte() {
            let input = vec![b'l'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            assert_eq!(byte_reader.read_byte().unwrap(), b'l');
            assert_eq!(byte_reader.input_byte_counter(), 1);
        }

        #[test]
        fn it_should_return_the_last_read_byte() {
            let input = vec![b'l', b'e'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            byte_reader.read_byte().unwrap();
            byte_reader.read_byte().unwrap();

            assert_eq!(byte_reader.last_byte(), Some(b'e'));
        }
    }

    mod for_peeking {
        use crate::rw::byte_reader::ByteReader;

        #[test]
        fn it_should_allow_peeking_one_byte_from_the_input_without_consuming_it() {
            let input = vec![b'l'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            assert_eq!(byte_reader.peek_byte().unwrap(), b'l');
            assert_eq!(byte_reader.peek_byte().unwrap(), b'l');
        }

        #[test]
        fn when_reading_a_byte_it_should_use_a_peeked_one_if_there_is() {
            let input = vec![b'l'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            assert_eq!(byte_reader.peek_byte().unwrap(), b'l');
            assert_eq!(byte_reader.read_byte().unwrap(), b'l');
        }

        #[test]
        fn when_reading_a_byte_it_should_use_a_peeked_one_and_discard_it_after_using_it() {
            let input = vec![b'l'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            assert_eq!(byte_reader.peek_byte().unwrap(), b'l'); // It peeks
            assert_eq!(byte_reader.read_byte().unwrap(), b'l'); // It uses the previously peeked byte
            assert!(byte_reader.peek_byte().is_err()); // There are no more bytes to peek
        }

        #[test]
        fn it_should_increase_the_input_byte_counter_the_first_time_it_peeks_a_new_byte() {
            let input = vec![b'l'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            assert_eq!(byte_reader.peek_byte().unwrap(), b'l');
            assert_eq!(byte_reader.input_byte_counter(), 1);
        }

        #[test]
        fn it_should_not_increase_the_input_byte_counter_when_peeking_a_cached_peeked_byte() {
            let input = vec![b'l'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            // It peeks the first time
            assert_eq!(byte_reader.peek_byte().unwrap(), b'l');
            assert_eq!(byte_reader.input_byte_counter(), 1);

            // It peeks the second time
            assert_eq!(byte_reader.peek_byte().unwrap(), b'l');
            assert_eq!(byte_reader.input_byte_counter(), 1);
        }
    }

    mod for_capturing {
        use crate::rw::byte_reader::ByteReader;

        #[test]
        fn it_should_capture_the_latest_read_byte() {
            let input = vec![b'a'];

            let mut byte_reader = ByteReader::new(input.as_slice());

            byte_reader.read_byte().unwrap();

            assert_eq!(byte_reader.captured_bytes(), input);
        }

        #[test]
        fn it_should_capture_1024_bytes_at_the_most() {
            let mut part1 = vec![b'a'; 1024];
            let part2 = vec![b'b'; 1024];
            part1.extend_from_slice(&part2);

            let mut byte_reader = ByteReader::new(part1.as_slice());

            for _i in 1..=1024 * 2 {
                byte_reader.read_byte().unwrap();
            }

            assert_eq!(byte_reader.captured_bytes(), part2);
        }
    }
}
