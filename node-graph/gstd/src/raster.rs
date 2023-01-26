use core::marker::PhantomData;
use dyn_any::{DynAny, StaticType};
use graphene_core::ops::{CloneNode, FlatMapResultNode};
use graphene_core::raster::{Color, Image};
use graphene_core::structural::{ComposeNode, ConsNode};
use graphene_core::{generic::FnNode, ops::MapResultNode, structural::Then, value::ValueNode, Node, NodeIO};
use image::Pixel;
use std::path::Path;

#[derive(Debug, DynAny)]
pub enum Error {
	IO(std::io::Error),
	Image(image::ImageError),
}

impl From<std::io::Error> for Error {
	fn from(e: std::io::Error) -> Self {
		Error::IO(e)
	}
}

pub trait FileSystem {
	fn open<P: AsRef<Path>>(&self, path: P) -> Result<Box<dyn std::io::Read>, Error>;
}

#[derive(Clone)]
pub struct StdFs;
impl FileSystem for StdFs {
	fn open<P: AsRef<Path>>(&self, path: P) -> Result<Reader, Error> {
		Ok(Box::new(std::fs::File::open(path)?))
	}
}
type Reader = Box<dyn std::io::Read>;

pub struct FileNode<FileSystem> {
	fs: FileSystem,
}
#[node_macro::node_fn(FileNode)]
fn file_node<P: AsRef<Path>, FS: FileSystem>(path: P, fs: FS) -> Result<Reader, Error> {
	Ok(fs.open(path)?)
}

pub struct BufferNode;
#[node_macro::node_fn(BufferNode)]
fn buffer_node<R: std::io::Read>(reader: R) -> Result<Vec<u8>, Error> {
	Ok(std::io::Read::bytes(reader).collect::<Result<Vec<_>, _>>()?)
}

/*
pub fn file_node<'i, 's: 'i, P: AsRef<Path> + 'i>() -> impl Node<'i, 's, P, Output = Result<Vec<u8>, Error>> {
	let fs = ValueNode(StdFs).then(CloneNode::new());
	let file = FileNode::new(fs);

	file.then(FlatMapResultNode::new(ValueNode::new(BufferNode)))
}

pub fn image_node<'i, 's: 'i, P: AsRef<Path> + 'i>() -> impl Node<'i, 's, P, Output = Result<Image, Error>> {
	let file = file_node();
	let image_loader = FnNode::new(|data: Vec<u8>| image::load_from_memory(&data).map_err(Error::Image).map(|image| image.into_rgba32f()));
	let image = file.then(FlatMapResultNode::new(ValueNode::new(image_loader)));
	let convert_image = FnNode::new(|image: image::ImageBuffer<_, _>| {
		let data = image
			.enumerate_pixels()
			.map(|(_, _, pixel): (_, _, &image::Rgba<f32>)| {
				let c = pixel.channels();
				Color::from_rgbaf32(c[0], c[1], c[2], c[3]).unwrap()
			})
			.collect();
		Image {
			width: image.width(),
			height: image.height(),
			data,
		}
	});

	image.then(MapResultNode::new(convert_image))
}

pub fn export_image_node<'i, 's: 'i>() -> impl Node<'i, 's, (Image, &'i str), Output = Result<(), Error>> {
	FnNode::new(|input: (Image, &str)| {
		let (image, path) = input;
		let mut new_image = image::ImageBuffer::new(image.width, image.height);
		for ((x, y, pixel), color) in new_image.enumerate_pixels_mut().zip((&image).data.iter()) {
			let color: Color = *color;
			assert!(x < image.width);
			assert!(y < image.height);
			*pixel = image::Rgba(color.to_rgba8())
		}
		new_image.save(path).map_err(Error::Image)
	})
}

#[derive(Debug, Clone, Copy)]
pub struct GrayscaleNode;

#[node_macro::node_fn(GrayscaleNode)]
fn grayscale_image(mut image: Image) -> Image {
	for pixel in &mut image.data {
		let avg = (pixel.r() + pixel.g() + pixel.b()) / 3.;
		*pixel = Color::from_rgbaf32_unchecked(avg, avg, avg, pixel.a());
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct InvertRGBNode;

#[node_macro::node_fn(InvertRGBNode)]
fn invert_image(mut image: Image) -> Image {
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(1. - pixel.r(), 1. - pixel.g(), 1. - pixel.b(), pixel.a());
	}
	image
}*/

#[derive(Debug, Clone, Copy)]
pub struct HueSaturationNode<Hue, Sat, Lit> {
	hue_shift: Hue,
	saturation_shift: Sat,
	lightness_shift: Lit,
}

#[node_macro::node_fn(HueSaturationNode)]
fn shift_image_hsl(image: Image, hue_shift: f64, saturation_shift: f64, lightness_shift: f64) -> Image {
	let mut image = image;
	let (hue_shift, saturation_shift, lightness_shift) = (hue_shift as f32, saturation_shift as f32, lightness_shift as f32);
	for pixel in &mut image.data {
		let [hue, saturation, lightness, alpha] = pixel.to_hsla();
		*pixel = Color::from_hsla(
			(hue + hue_shift / 360.) % 1.,
			(saturation + saturation_shift / 100.).clamp(0., 1.),
			(lightness + lightness_shift / 100.).clamp(0., 1.),
			alpha,
		);
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct BrightnessContrastNode<Brightness, Contrast> {
	brightness: Brightness,
	contrast: Contrast,
}

// From https://stackoverflow.com/questions/2976274/adjust-bitmap-image-brightness-contrast-using-c
#[node_macro::node_fn(BrightnessContrastNode)]
fn adjust_image_brightness_and_contrast(image: Image, brightness: f64, contrast: f64) -> Image {
	let mut image = image;
	let (brightness, contrast) = (brightness as f32, contrast as f32);
	let factor = (259. * (contrast + 255.)) / (255. * (259. - contrast));
	let channel = |channel: f32| ((factor * (channel * 255. + brightness - 128.) + 128.) / 255.).clamp(0., 1.);

	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct GammaNode<G> {
	gamma: G,
}

// https://www.dfstudios.co.uk/articles/programming/image-programming-algorithms/image-processing-algorithms-part-6-gamma-correction/
#[node_macro::node_fn(GammaNode)]
fn image_gamma(image: Image, gamma: f64) -> Image {
	let mut image = image;
	let inverse_gamma = 1. / gamma;
	let channel = |channel: f32| channel.powf(inverse_gamma as f32);
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct OpacityNode<O> {
	opacity_multiplier: O,
}

#[node_macro::node_fn(OpacityNode)]
fn image_opacity(image: Image, opacity_multiplier: f64) -> Image {
	let mut image = image;
	let opacity_multiplier = opacity_multiplier as f32;
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(pixel.r(), pixel.g(), pixel.b(), pixel.a() * opacity_multiplier)
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct PosterizeNode<P> {
	posterize_value: P,
}

// Based on http://www.axiomx.com/posterize.htm
#[node_macro::node_fn(PosterizeNode)]
fn posterize(image: Image, posterize_value: f64) -> Image {
	let mut image = image;
	let posterize_value = posterize_value as f32;
	let number_of_areas = posterize_value.recip();
	let size_of_areas = (posterize_value - 1.).recip();
	let channel = |channel: f32| (channel / number_of_areas).floor() * size_of_areas;
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct ExposureNode<E> {
	exposure: E,
}

// Based on https://stackoverflow.com/questions/12166117/what-is-the-math-behind-exposure-adjustment-on-photoshop
#[node_macro::node_fn(ExposureNode)]
fn exposure(image: Image, exposure: f64) -> Image {
	let mut image = image;
	let multiplier = 2f32.powf(exposure as f32);
	let channel = |channel: f32| channel * multiplier;
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct ImaginateNode<E> {
	cached: E,
}

// Based on https://stackoverflow.com/questions/12166117/what-is-the-math-behind-exposure-adjustment-on-photoshop
#[node_macro::node_fn(ImaginateNode)]
fn imaginate(image: Image, cached: Option<std::sync::Arc<graphene_core::raster::Image>>) -> Image {
	info!("Imaginating image with {} pixels", image.data.len());
	cached.map(|mut x| std::sync::Arc::make_mut(&mut x).clone()).unwrap_or(image)
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn load_image() {
		// TODO: reenable this test
		/*
		let image = image_node::<&str>();

		let grayscale_picture = image.then(MapResultNode::new(&image));
		let export = export_image_node();

		let picture = grayscale_picture.eval("test-image-1.png").expect("Failed to load image");
		export.eval((picture, "test-image-1-result.png")).unwrap();
		*/
	}
}
