use crate::Node;
use graphene_core::raster::{ImageFrame, SRGBA8};

pub struct GetNode;

#[node_macro::node_fn(GetNode)]
async fn get_node(url: String) -> reqwest::Response {
	reqwest::get(url).await.unwrap()
}

pub struct PostNode<Body> {
	body: Body,
}

#[node_macro::node_fn(PostNode)]
async fn post_node(url: String, body: String) -> reqwest::Response {
	reqwest::Client::new().post(url).body(body).send().await.unwrap()
}

#[cfg(feature = "serde")]
async fn image_to_image(image: ImageFrame<SRGBA8>, prompt: String) -> reqwest::Result<ImageFrame<SRGBA8>> {
	let png_bytes = image.image.to_png();
	// let base64 = base64::encode(png_bytes);
	// post to cloudflare image to image endpoint using reqwest
	let payload = PayloadBuilder::new().guidance(7.5).image(png_bytes.to_vec()).num_steps(20).prompt(prompt).strength(1).build();

	let client = Client::new();
	let account_id = "023e105f4ecef8ad9ca31a8372d0c353";
	let response = client
		.post(format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/bytedance/stable-diffusion-xl-lightning"))
		.json(&payload)
		.header("Content-Type", "application/json")
		.header("Authorization", "Bearer 123")
		.send()
		.await?;

	let text = response.text().await?;
	println!("{}", text);

	Ok(image)
}
use reqwest::Client;
use serde::Serialize;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Default)]
struct PayloadBuilder {
	guidance: Option<f64>,
	image: Option<Vec<u8>>,
	mask: Option<Vec<u32>>,
	num_steps: Option<u32>,
	prompt: Option<String>,
	strength: Option<u32>,
}

impl PayloadBuilder {
	fn new() -> Self {
		Self::default()
	}

	fn guidance(mut self, value: f64) -> Self {
		self.guidance = Some(value);
		self
	}

	fn image(mut self, value: Vec<u8>) -> Self {
		self.image = Some(value);
		self
	}

	fn mask(mut self, value: Vec<u32>) -> Self {
		self.mask = Some(value);
		self
	}

	fn num_steps(mut self, value: u32) -> Self {
		self.num_steps = Some(value);
		self
	}

	fn prompt(mut self, value: String) -> Self {
		self.prompt = Some(value);
		self
	}

	fn strength(mut self, value: u32) -> Self {
		self.strength = Some(value);
		self
	}

	fn build(self) -> Payload {
		Payload {
			guidance: self.guidance.unwrap_or_default(),
			image: self.image.unwrap_or_default(),
			mask: self.mask.unwrap_or_default(),
			num_steps: self.num_steps.unwrap_or_default(),
			prompt: self.prompt.unwrap_or_default(),
			strength: self.strength.unwrap_or_default(),
		}
	}
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct Payload {
	guidance: f64,
	image: Vec<u8>,
	mask: Vec<u32>,
	num_steps: u32,
	prompt: String,
	strength: u32,
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::{raster::Image, Color};
	#[tokio::test]
	async fn test_cloudflare() {
		let test_image = ImageFrame {
			image: Image::new(100, 100, SRGBA8::from(Color::RED)),
			..Default::default()
		};
		let result = image_to_image(test_image, "make green".into()).await;
		dbg!(result);
		panic!("show result");
	}
}
