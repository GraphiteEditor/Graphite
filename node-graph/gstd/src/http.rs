use graphene_core::raster::{ImageFrame, SRGBA8};

use crate::Node;

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

async fn image_to_image(image: ImageFrame<SRGBA8>, prompt: String) -> ImageFrame<SRGBA8> {
	let png_bytes = image.image.to_png();
	let base64 = base64::encode(png_bytes);
	// post to cloudflare image to image endpoint using reqwest

	image
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::{raster::Image, Color};
	#[test]
	fn test_cloudflare() {
		let test_image = ImageFrame {
			image: Image::new(100, 100, SRGBA8::from(Color::RED)),
			..Default::default()
		};
		let result = futures::executor::block_on(image_to_image(test_image, "make green".into()));
		dbg!(result);
		panic!("show result");
	}
}
