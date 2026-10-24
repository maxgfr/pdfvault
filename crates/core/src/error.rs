use std::fmt;

/// Every failure the engine reports. `code()` is stable and is what the
/// TypeScript engine, the CLI's `--json` output and the web UI switch on.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a readable PDF: {0}")]
    InvalidPdf(String),
    #[error("this PDF is password-protected; supply its password")]
    PasswordRequired,
    #[error("wrong password")]
    WrongPassword,
    #[error("input too large: {0}")]
    Limit(String),
    #[error("invalid option: {0}")]
    InvalidOption(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image error: {0}")]
    Image(String),
    /// Characters none of the given fonts can draw, in the order met.
    #[error("no font has these characters: {0}")]
    MissingGlyphs(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::InvalidPdf(_) => "invalid_pdf",
            Error::PasswordRequired => "password_required",
            Error::WrongPassword => "wrong_password",
            Error::Limit(_) => "limit",
            Error::InvalidOption(_) => "invalid_option",
            Error::Unsupported(_) => "unsupported",
            Error::Image(_) => "image",
            Error::MissingGlyphs(_) => "missing_glyphs",
        }
    }

    pub fn option(msg: impl fmt::Display) -> Self {
        Error::InvalidOption(msg.to_string())
    }
}

impl From<lopdf::Error> for Error {
    fn from(e: lopdf::Error) -> Self {
        match e {
            lopdf::Error::InvalidPassword => Error::WrongPassword,
            other => Error::InvalidPdf(other.to_string()),
        }
    }
}

impl From<image::ImageError> for Error {
    fn from(e: image::ImageError) -> Self {
        Error::Image(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::InvalidOption(e.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
