use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] edmc_core::Error),

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("url: {0}")]
    Url(#[from] url::ParseError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("Too many requests. Please wait a while and try again.")]
    RateLimited,

    #[error("The site is busy ({0}). Please wait a while and try again.")]
    Busy(String),

    #[error("could not parse the page: {0}")]
    Parse(String),

    #[error("not configured: {0}")]
    NotConfigured(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
