//! pdfvault-core: the PDF and image engine shared by the pdfvault site and CLI.
//! Pure Rust, no I/O: bytes in, bytes and a report out. [`dispatch::run`] is
//! the single entry point every front end goes through.

pub mod compress;
pub mod content;
pub mod dispatch;
pub mod edit;
pub mod epub;
pub mod error;
pub mod forms;
mod glyphlist;
pub mod images;
pub mod info;
pub mod interp;
pub mod kdf;
pub mod markdown;
pub mod meta;
pub mod pages;
pub mod pdf;
pub mod pdffont;
pub mod raster;
pub mod redact;
pub mod scrub;
pub mod security;
pub mod stamp;
mod std14;
pub mod textedit;
pub mod textlayer;
#[cfg(feature = "typeset")]
pub mod typeset;

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
#[cfg(test)]
mod testutil;

pub use error::{Error, Result};
pub use lopdf;
