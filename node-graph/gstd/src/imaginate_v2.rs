use graphene_core::raster::{Image, ImageFrame, Pixel, SRGBA8};

use crate::Node;

async fn image_to_image(image_frame: ImageFrame<SRGBA8>, prompt: String) -> reqwest::Result<ImageFrame<SRGBA8>> {
	let png_bytes = image_frame.image.to_png();
	//let base64 = base64::encode(png_bytes);
	// post to cloudflare image to image endpoint using reqwest
	let payload = PayloadBuilder::new()
		.guidance(7.5)
		.image(png_bytes.to_vec())
		//.mask(png_bytes.to_vec())
		.num_steps(20)
		.prompt(prompt)
		.strength(1);

	let client = Client::new();
	let account_id = "xxx";
	let api_key = "123";
	let request = client
		//.post(format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/bytedance/stable-diffusion-xl-base-1.0"))
		//.post(format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/stabilityai/stable-diffusion-xl-base-1.0"))
		/*.post(format!(
			"https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/runwayml/stable-diffusion-v1-5-inpainting"
		))*/
		.post(format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/@cf/runwayml/stable-diffusion-v1-5-img2img"))
		.json(&payload)
		.header("Authorization", format!("Bearer {api_key}"));
	//println!("{}", serde_json::to_string(&payload).unwrap());
	let response = dbg!(request).send().await?;

	#[derive(Debug, serde::Deserialize)]
	struct Response {
		result: String,
		success: bool,
	};

	match response.error_for_status_ref() {
		Ok(_) => (),
		Err(_) => panic!("{}", response.text().await?),
	}
	//let text: Response = response.json().await?;
	/*let text = response.text().await?;
	let text = Response {
		result: serde_json::Value::String(text),
		success: false,
	};
	dbg!(&text);*/

	let bytes = response.bytes().await?;
	//let bytes = &[];

	let image = image::load_from_memory_with_format(&bytes[..], image::ImageFormat::Png).unwrap();
	let width = image.width();
	let height = image.height();
	let image = image.to_rgba8();
	let data = image.as_raw();
	let color_data = bytemuck::cast_slice(data).to_owned();
	let image = Image {
		width,
		height,
		data: color_data,
		base64_string: None,
	};

	let image_frame = ImageFrame { image, ..image_frame };
	Ok(image_frame)
}
use reqwest::Client;
use serde::Serialize;

#[derive(Default, Serialize)]
struct PayloadBuilder {
	#[serde(skip_serializing_if = "Option::is_none")]
	guidance: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	image: Option<Vec<u8>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	mask: Option<Vec<u8>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	num_steps: Option<u32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	prompt: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
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

	fn mask(mut self, value: Vec<u8>) -> Self {
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
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::{raster::Image, Color};
	#[tokio::test]
	async fn test_cloudflare() {
		let test_image = ImageFrame {
			image: Image::new(1024, 1024, SRGBA8::from(Color::RED)),
			..Default::default()
		};
		let result = image_to_image(test_image, "make green".into()).await;
		dbg!(result.unwrap());
		panic!("show result");
	}
}
