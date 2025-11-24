use std::collections::HashMap;
use std::io::Write;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Error;
use crate::Pointer;

mod http;

pub use http::HttpClient;

#[async_trait]
pub trait Download: Send + Sync {
  async fn download(self, to: &mut impl Write) -> Result<usize, Error>;
}

#[async_trait]
pub trait BatchDownload: Send + Sync {
  async fn batch_download(self, to: &mut impl Write) -> Result<usize, Error>;
}

#[async_trait]
pub trait RemoteClient: Send + Sync {
  async fn batch(&self, request: BatchRequest) -> Result<BatchResponse, Error>;
  async fn download(&self, action: &ObjectAction) -> Result<Vec<u8>, Error>;
}

pub struct LfsRemote<'a> {
  repo: &'a git2::Repository,
  client: &'a dyn RemoteClient,
}

#[derive(Serialize)]
pub struct BatchRequest {
  pub operation: &'static str,
  pub transfers: Vec<&'static str>,
  pub objects: Vec<BatchObject>,
}

#[derive(Serialize)]
pub struct BatchObject {
  pub oid: String,
  pub size: usize,
}

impl BatchRequest {
  fn from_pointers(pointers: &[Pointer]) -> Self {
    Self {
      operation: "download",
      transfers: vec!["basic"],
      objects: pointers
        .iter()
        .map(|p| BatchObject { oid: format!("sha256:{}", p.hex()), size: p.size() })
        .collect(),
    }
  }
}

#[derive(Deserialize)]
pub struct BatchResponse {
  pub objects: Vec<BatchResponseObject>,
}

#[derive(Deserialize)]
pub struct BatchResponseObject {
  pub oid: String,
  pub size: usize,
  pub actions: Option<ObjectActionSet>,
}

#[derive(Deserialize)]
pub struct ObjectActionSet {
  pub download: ObjectAction,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ObjectAction {
  pub href: String,

  #[serde(default)]
  pub header: HashMap<String, String>,
}

impl<'a> LfsRemote<'a> {
  pub fn new(repo: &'a git2::Repository, client: &'a dyn RemoteClient) -> Self {
    Self { repo, client }
  }

  pub async fn pull(&self, pointers: &[Pointer]) -> Result<(), Error> {
    if pointers.is_empty() {
      return Ok(());
    }

    let request = BatchRequest::from_pointers(pointers);
    let response = self.client.batch(request).await?;

    for object in response.objects {
      let actions = object.actions.ok_or(Error::EmptyResponse)?;
      let bytes = self.client.download(&actions.download).await?;

      let oid_hex = object.oid.strip_prefix("sha256:").unwrap_or(&object.oid);
      let pointer =
        pointers.iter().find(|p| p.hex() == oid_hex).ok_or(Error::Remote("pointer not found".to_string()))?;

      validate_checksum(pointer, &bytes)?;

      let object_dir = self.repo.path().join("lfs/objects");
      pointer.write_blob_bytes(&object_dir, &bytes)?;
    }

    Ok(())
  }
}

fn validate_checksum(pointer: &Pointer, bytes: &[u8]) -> Result<(), Error> {
  if bytes.len() != pointer.size() {
    return Err(Error::ChecksumMismatch);
  }

  let mut hasher = Sha256::new();
  hasher.update(bytes);
  let hash = hasher.finalize();

  if hash.as_slice() != pointer.hash() {
    return Err(Error::ChecksumMismatch);
  }

  Ok(())
}
