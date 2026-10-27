#![forbid(unsafe_code)]

pub mod ast;
pub mod content;
pub mod decoders;
pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod tree;
pub mod value;
pub mod widgets;

pub use ast::{Library, Part};
pub use content::DynamicContent;
pub use error::{ParseError, ParseErrorKind};
pub use eval::{PLUGIN_LIBRARY, evaluate};
pub use parser::{parse_data_file, parse_library_file};
pub use tree::{Child, Event, LocalLibrary, Node, Widget};
pub use value::{Map, Value};

pub fn parse_error_text(error: &ParseError) -> String {
    format!("a interface do plugin não leu:\n{error}")
}
