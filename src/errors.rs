use thiserror::Error;

#[derive(Debug, Error)]
pub enum UsbError {
    #[error("USB device not found")]
    NotFound,
    #[error("Access denied (permissions)")]
    AccessDenied,
    #[error("Device busy")]
    Busy,
    #[error("Timeout")]
    Timeout,
    #[error("I/O error")]
    Io,
    #[error("Bad password")]
    BadPassword,
    #[error("{0}")]
    Other(String),
}
