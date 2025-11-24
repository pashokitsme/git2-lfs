use crate::Pointer;
use crate::remote::Write;
use crate::remote::dto::BatchResponse;

use reqwest::header::HeaderMap;
use url::Url;

use sha2::Digest;
use sha2::Sha256;

use async_trait::async_trait;

use crate::remote::Download;
use crate::remote::RemoteError;

use crate::remote::dto::*;

pub struct ReqwestLfsClient {
  client: reqwest::Client,
  url: Url,
  headers: Option<HeaderMap>,
}

impl ReqwestLfsClient {
  pub fn new(base_url: Url, access_token: Option<&str>) -> Result<Self, RemoteError> {
    let mut url = base_url;

    if let Some(token) = access_token {
      url
        .set_username("oauth2")
        .map_err(|_| RemoteError::UrlParse(url::ParseError::RelativeUrlWithoutBase))?;
      url
        .set_password(Some(token))
        .map_err(|_| RemoteError::UrlParse(url::ParseError::RelativeUrlWithoutBase))?;
    }

    Ok(Self { client: reqwest::Client::new(), url, headers: None })
  }

  pub fn headers(self, headers: HeaderMap) -> Self {
    Self { headers: Some(headers), ..self }
  }
}

#[async_trait]
impl Download for ReqwestLfsClient {
  async fn batch(&self, req: BatchRequest) -> Result<BatchResponse, RemoteError> {
    todo!()
  }

  async fn download(&self, action: &ObjectAction, to: &mut Write) -> Result<Pointer, RemoteError> {
    use futures::StreamExt;

    let mut req = self.client.get(self.url.clone());

    if let Some(headers) = &self.headers {
      req = req.headers(headers.clone());
    }

    let res = req.send().await.map_err(|e| RemoteError::Custom(Box::new(e)))?;

    if !res.status().is_success() {
      use reqwest::StatusCode as S;

      return match res.status() {
        S::FORBIDDEN => Err(RemoteError::AccessDenied),
        S::NOT_FOUND => Err(RemoteError::NotFound),
        _ => Err(RemoteError::Custom(Box::new(res.error_for_status().unwrap_err()))),
      };
    }

    let mut bytes = res.bytes_stream();
    let mut total = 0;

    let mut checksum = Sha256::new();

    while let Some(bytes) = bytes.next().await {
      let bytes = bytes.map_err(|e| RemoteError::Download(crate::report_error(&e)))?;
      total += to.write(&bytes)?;
      checksum.update(&bytes);
    }

    let hash = checksum.finalize();

    Ok(Pointer::from_parts(hash.as_slice(), total))
  }
}
