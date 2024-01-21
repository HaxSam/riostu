use thiserror::Error;

#[derive(Error, Debug)]
pub enum RemoteIoError {
    #[error("")]
    Request(#[from] isahc::Error),
    #[error("")]
    NotSeekable,
    #[error("")]
    NoContentSize
}

