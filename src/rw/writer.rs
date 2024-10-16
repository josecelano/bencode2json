//! This module contains the `Writer` trait.

/* code-review:

   The function `write_byte` only writes:

   - Bytes used in integers:
     - Digits: '0','1','2','3','4','5','6','7','8','9'
     - 'e', '-'
   - JSON reservers chars: '[', ',', ']', '{', ',', ':', '}' defined as constants.

   It could be refactored to be more restrictive. However, in the future we also
   want to print Bencoded strings as bytes streams, without trying to convert
   them into UTF-8 strings.
*/

use super::error::Error;

pub trait Writer {
    /// It writes one byte to the output.
    ///
    /// # Errors
    ///
    /// Will return an error if it can't write the byte.
    fn write_byte(&mut self, byte: u8) -> Result<(), Error>;

    /// It writes a string to the output.
    ///
    /// # Errors
    ///
    /// Will return an error if it can't write the string.
    fn write_str(&mut self, value: &str) -> Result<(), Error>;

    /// It return the number of bytes that have been written to the output.
    fn output_byte_counter(&self) -> u64;

    /// It returns a copy of the latest bytes that have been written to the
    /// output.
    fn captured_bytes(&self) -> Vec<u8>;
}
