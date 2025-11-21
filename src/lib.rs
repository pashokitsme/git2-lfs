pub mod ext;
pub mod remote;

mod lfs;
mod pointer;

pub use pointer::Pointer;

pub use lfs::Lfs;
pub use lfs::LfsBuilder;

#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[error("resulting hash should be exactly 32 bytes, got {0}")]
  InvalidHashLength(usize),

  #[error("the pointer containts invalid spec, expected '{expected}', got '{actual}'")]
  InvalidSpec { expected: String, actual: String },

  #[error("the pointer containts invalid size: '{0}'")]
  InvalidSize(String),

  #[error("not a pointer")]
  NotAPointer,

  #[error(transparent)]
  Utf8(#[from] std::str::Utf8Error),

  #[error("hex: {0}")]
  Hex(#[from] hex::FromHexError),

  #[error(transparent)]
  Git2(#[from] git2::Error),

  #[error("io: {0}")]
  Io(#[from] std::io::Error),
}
