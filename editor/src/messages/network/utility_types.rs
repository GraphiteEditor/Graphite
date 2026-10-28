use graph_craft::application_io::http;

#[derive(Debug, Clone, Default)]
pub struct Client;

impl Client {
	pub async fn fetch(&self, url: &str) -> Option<Box<[u8]>> {
		let url = url.parse().map_err(|err| log::error!("failed to fetch: {err}")).ok()?;
		let mut request = reqwest::Request::new(reqwest::Method::GET, url);
		if cfg!(not(target_family = "wasm")) {
			*request.timeout_mut() = Some(std::time::Duration::from_secs(100));
		}
		let response = http::send(request).await.map_err(|err| log::error!("failed to fetch: {err}")).ok()?;
		let response = response.error_for_status().map_err(|err| log::error!("failed to fetch: {err}")).ok()?;
		let bytes = response.bytes().await.map_err(|err| log::error!("failed to read response body: {err}")).ok()?;
		Some(bytes.to_vec().into_boxed_slice())
	}
}
