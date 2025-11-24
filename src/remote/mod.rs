mod reqwest;

use crate::Pointer;
use std::fs::File;

use async_trait::async_trait;

use sha2::Digest;
use sha2::Sha256;

pub use dto::*;

mod dto;

#[derive(thiserror::Error, Debug)]
pub enum RemoteError {
  #[error("access denied")]
  AccessDenied,

  #[error("not found")]
  NotFound,

  #[error("download failed: {0}")]
  Download(String),

  #[error("checksum mismatch")]
  ChecksumMismatch,

  #[error("empty response")]
  EmptyResponse,

  #[error("url parse error: {0}")]
  UrlParse(#[from] url::ParseError),

  #[error("io: {0}")]
  Io(#[from] std::io::Error),

  #[error("{}", crate::report_error(self))]
  Custom(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub type Write = dyn std::io::Write + Send;

#[async_trait]
pub trait Download: Send + Sync {
  async fn batch(&self, req: BatchRequest) -> Result<BatchResponse, RemoteError>;
  async fn download(&self, action: &ObjectAction, to: &mut Write) -> Result<Pointer, RemoteError>;
}

pub struct LfsRemote<'a, C: Send + Sync> {
  repo: &'a git2::Repository,
  client: C,
}

impl<'a, C: Download + Send + Sync> LfsRemote<'a, C> {
  pub fn new(repo: &'a git2::Repository, client: C) -> Self {
    Self { repo, client }
  }

  pub async fn pull(&self, pointers: &[&Pointer]) -> Result<(), RemoteError> {
    // self.client.download(&mut File::create("test")?).await?;
    todo!()
  }
}

fn validate_checksum(pointer: &Pointer, bytes: &[u8]) -> Result<(), RemoteError> {
  if bytes.len() != pointer.size() {
    return Err(RemoteError::ChecksumMismatch);
  }

  let mut hasher = Sha256::new();
  hasher.update(bytes);
  let hash = hasher.finalize();

  if hash.as_slice() != pointer.hash() {
    return Err(RemoteError::ChecksumMismatch);
  }

  Ok(())
}
