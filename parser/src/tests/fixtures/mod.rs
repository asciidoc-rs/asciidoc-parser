pub(crate) mod attributes;
pub(crate) mod blocks;
pub(crate) mod content;
pub(crate) mod document;
pub(crate) mod image_file_handler;
pub(crate) mod inline_file_handler;
pub(crate) mod parser;
pub(crate) mod svg_file_handler;

mod span;
pub(crate) use span::Span;

pub(crate) mod warnings;
