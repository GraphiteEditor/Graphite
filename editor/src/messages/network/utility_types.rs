use reqwest::IntoUrl;

#[derive(Debug, Default, Clone)]
pub struct Client {
	inner: reqwest::Client,
}

impl Client {
	pub async fn fetch<U: IntoUrl>(&self, url: U) -> Option<Box<[u8]>> {
		match self.inner.get(url).send().await {
			Ok(response) => match response.bytes().await {
				Ok(bytes) => Some(bytes.to_vec().into_boxed_slice()),
				Err(err) => {
					log::error!("failed to read response body: {err}");
					None
				}
			},
			Err(err) => {
				log::error!("failed to fetch: {err}");
				None
			}
		}
	}
}
