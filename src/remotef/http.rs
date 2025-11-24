use async_trait::async_trait;
use reqwest::Client;
use url::Url;

use crate::Error;
use crate::remotef::{BatchRequest, BatchResponse, ObjectAction, RemoteClient};

const MEDIA_TYPE: &str = "application/vnd.git-lfs+json";

pub struct HttpClient {
  client: Client,
  base_url: String,
  access_token: Option<String>,
}

impl HttpClient {
  pub fn new(base_url: String, access_token: Option<String>) -> Self {
    Self { client: Client::builder().build().expect("failed to build http client"), base_url, access_token }
  }

  fn url_with_auth(&self, url: &str) -> Result<Url, Error> {
    let mut url = Url::parse(url)?;
    if let Some(token) = &self.access_token {
      url.set_username("oauth2").map_err(|_| Error::Remote("invalid url".to_string()))?;
      url.set_password(Some(token)).map_err(|_| Error::Remote("invalid url".to_string()))?;
    }
    Ok(url)
  }
}

#[async_trait]
impl RemoteClient for HttpClient {
  async fn batch(&self, request: BatchRequest) -> Result<BatchResponse, Error> {
    let url = format!("{}/info/lfs/objects/batch", self.base_url);
    let url = self.url_with_auth(&url)?;

    let req =
      self.client.post(url).header("Accept", MEDIA_TYPE).header("Content-Type", MEDIA_TYPE).json(&request);

    let response = req.send().await.map_err(|e| Error::Remote(e.to_string()))?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await.unwrap_or_default();
      return Err(Error::Remote(format!("batch request failed: {} - {}", status, body)));
    }

    let result = response.json::<BatchResponse>().await.map_err(|e| Error::Remote(e.to_string()))?;
    Ok(result)
  }

  async fn download(&self, action: &ObjectAction) -> Result<Vec<u8>, Error> {
    let url = self.url_with_auth(&action.href)?;

    let mut req = self.client.get(url);

    for (key, value) in &action.header {
      req = req.header(key, value);
    }

    let response = req.send().await.map_err(|e| Error::Remote(e.to_string()))?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await.unwrap_or_default();
      return Err(Error::Remote(format!("download failed: {} - {}", status, body)));
    }

    let bytes = response.bytes().await.map_err(|e| Error::Remote(e.to_string()))?;
    Ok(bytes.to_vec())
  }
}
