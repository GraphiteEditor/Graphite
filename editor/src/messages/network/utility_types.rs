use reqwest::IntoUrl;

#[derive(Debug, Clone)]
pub struct Client {
	inner: reqwest::Client,
}

impl Default for Client {
	fn default() -> Self {
		Self {
			#[cfg(not(target_family = "wasm"))]
			inner: reqwest::Client::builder().timeout(std::time::Duration::from_secs(100)).build().unwrap_or_default(),
			#[cfg(target_family = "wasm")]
			inner: reqwest::Client::new(),
		}
	}
}

impl Client {
	pub async fn fetch<U: IntoUrl>(&self, url: U) -> Option<Box<[u8]>> {
		let response = self.inner.get(url).send().await;
		let response = response.and_then(|r| r.error_for_status()).map_err(|err| log::error!("failed to fetch: {err}")).ok()?;
		let bytes = response.bytes().await.map_err(|err| log::error!("failed to read response body: {err}")).ok()?;
		Some(bytes.to_vec().into_boxed_slice())
	}
}
