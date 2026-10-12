//! pdfvault-core: the PDF and image engine shared by the pdfvault site and CLI.
//! Pure Rust, no I/O: bytes in, bytes and a report out. [`dispatch::run`] is
//! the single entry point every front end goes through.

pub mod compress;
pub mod dispatch;
pub mod epub;
pub mod error;
pub mod forms;
pub mod images;
pub mod info;
pub mod kdf;
pub mod markdown;
pub mod meta;
pub mod pages;
pub mod pdf;
pub mod raster;
pub mod security;
pub mod stamp;
pub mod textlayer;

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
#[cfg(test)]
mod testutil;

pub use error::{Error, Result};
pub use lopdf;
