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

use crate::remote::MEDIA_TYPE;
use crate::remote::dto::*;

pub struct ReqwestLfsClient {
  client: reqwest::Client,
  url: Url,
  access_token: Option<String>,
  headers: Option<HeaderMap>,
}

impl ReqwestLfsClient {
  pub fn new(url: Url, access_token: Option<String>) -> Self {
    Self { client: reqwest::Client::new(), url, access_token, headers: None }
  }

  pub fn headers(self, headers: HeaderMap) -> Self {
    Self { headers: Some(headers), ..self }
  }
}

#[async_trait]
impl Download for ReqwestLfsClient {
  async fn batch(&self, req: BatchRequest) -> Result<BatchResponse, RemoteError> {
    let mut batch_url = self.url.clone();
    batch_url
      .path_segments_mut()
      .map_err(|_| RemoteError::UrlParse(url::ParseError::RelativeUrlWithoutBase))?
      .pop_if_empty()
      .push("objects")
      .push("batch");

    let mut request =
      self.client.post(batch_url).header("Accept", MEDIA_TYPE).header("Content-Type", MEDIA_TYPE).json(&req);

    if let Some(token) = &self.access_token {
      request = request.basic_auth("oauth2", Some(token));
    }

    if let Some(headers) = &self.headers {
      request = request.headers(headers.clone());
    }

    let response = request.send().await.map_err(|e| RemoteError::Custom(Box::new(e)))?;

    if !response.status().is_success() {
      use reqwest::StatusCode as S;

      return match response.status() {
        S::FORBIDDEN => Err(RemoteError::AccessDenied),
        S::NOT_FOUND => Err(RemoteError::NotFound),
        _ => {
          let status = response.status();
          let body = response.text().await.unwrap_or_default();
          Err(RemoteError::Download(format!("batch request failed: {} - {}", status, body)))
        }
      };
    }

    let result = response.json::<BatchResponse>().await.map_err(|e| RemoteError::Custom(Box::new(e)))?;

    if result.objects.is_empty() {
      return Err(RemoteError::EmptyResponse);
    }

    Ok(result)
  }

  async fn download(&self, action: &ObjectAction, to: &mut Write) -> Result<Pointer, RemoteError> {
    use futures::StreamExt;

    let mut req = self.client.get(&action.href);

    for (key, value) in action.header.iter() {
      req = req.header(key, value);
    }

    let res = req.send().await.map_err(|e| RemoteError::Custom(Box::new(e)))?;

    if !res.status().is_success() {
      use reqwest::StatusCode as S;

      return match res.status() {
        S::FORBIDDEN => Err(RemoteError::AccessDenied),
        S::NOT_FOUND => Err(RemoteError::NotFound),
        _ => {
          let status = res.status();
          let body = res.text().await.unwrap_or_default();
          Err(RemoteError::Download(format!("download failed: {} - {}", status, body)))
        }
      };
    }

    let mut bytes = res.bytes_stream();
    let mut total = 0;

    let mut checksum = Sha256::new();

    while let Some(chunk) = bytes.next().await {
      let chunk = chunk.map_err(|e| RemoteError::Download(crate::report_error(&e)))?;
      total += to.write(&chunk)?;
      checksum.update(&chunk);
    }

    let hash = checksum.finalize();

    Ok(Pointer::from_parts(hash.as_slice(), total))
  }
}
