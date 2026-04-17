use thiserror::Error;

#[derive(Debug, Error)]
pub enum PacketError {
    #[error("packet too small: need {needed} bytes, have {have}")]
    TooSmall { needed: usize, have: usize },

    #[error("declared packet size {declared} does not match available data {available}")]
    SizeMismatch { declared: usize, available: usize },

    #[error("blowfish input length {0} is not a multiple of 8")]
    BlowfishBlockMisaligned(usize),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
