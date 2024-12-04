pub mod json;
pub mod stack;

use derive_more::derive::Display;

#[derive(Debug, PartialEq, Display)]
pub enum BencodeType {
    Integer,
    String,
    List,
    Dict,
}
