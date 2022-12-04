use core::marker::PhantomData;
use dyn_any::{DynAny, StaticType};
use graphene_core::ops::FlatMapResultNode;
use graphene_core::raster::color::Color;
use graphene_core::structural::{ComposeNode, ConsNode};
use graphene_core::{generic::FnNode, ops::MapResultNode, structural::Then, value::ValueNode, Node};
use image::Pixel;
use std::path::Path;

pub struct MapNode<MN: Node<S>, I: IntoIterator<Item = S>, S>(pub MN, PhantomData<(S, I)>);

impl<I: IntoIterator<Item = S>, MN: Node<S> + Copy, S> Node<I> for MapNode<MN, I, S> {
	type Output = Vec<MN::Output>;
	fn eval(self, input: I) -> Self::Output {
		input.into_iter().map(|x| self.0.eval(x)).collect()
	}
}

impl<I: IntoIterator<Item = S>, MN: Node<S>, S> MapNode<MN, I, S> {
	pub const fn new(mn: MN) -> Self {
		MapNode(mn, PhantomData)
	}
}

pub struct MapImageNode<MN: Node<Color, Output = Color> + Copy>(pub MN);

impl<MN: Node<Color, Output = Color> + Copy> Node<Image> for MapImageNode<MN> {
	type Output = Image;
	fn eval(self, input: Image) -> Self::Output {
		Image {
			width: input.width,
			height: input.height,
			data: input.data.iter().map(|x| self.0.eval(*x)).collect(),
		}
	}
}

impl<'n, MN: Node<Color, Output = Color> + Copy> Node<Image> for &'n MapImageNode<MN> {
	type Output = Image;
	fn eval(self, input: Image) -> Self::Output {
		Image {
			width: input.width,
			height: input.height,
			data: input.data.iter().map(|x| self.0.eval(*x)).collect(),
		}
	}
}

impl<MN: Node<Color, Output = Color> + Copy> MapImageNode<MN> {
	pub const fn new(mn: MN) -> Self {
		MapImageNode(mn)
	}
}

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

pub struct FileNode<P: AsRef<Path>, FS: FileSystem>(PhantomData<(P, FS)>);
impl<P: AsRef<Path>, FS: FileSystem> Node<(P, FS)> for FileNode<P, FS> {
	type Output = Result<Reader, Error>;

	fn eval(self, input: (P, FS)) -> Self::Output {
		let (path, fs) = input;
		fs.open(path)
	}
}

pub struct BufferNode;
impl<Reader: std::io::Read> Node<Reader> for BufferNode {
	type Output = Result<Vec<u8>, Error>;

	fn eval(self, mut reader: Reader) -> Self::Output {
		let mut buffer = Vec::new();
		reader.read_to_end(&mut buffer)?;
		Ok(buffer)
	}
}

#[derive(Clone, Debug, PartialEq, DynAny, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image {
	pub width: u32,
	pub height: u32,
	pub data: Vec<Color>,
}

impl Image {
	pub const fn empty() -> Self {
		Self {
			width: 0,
			height: 0,
			data: Vec::new(),
		}
	}
}

impl IntoIterator for Image {
	type Item = Color;
	type IntoIter = std::vec::IntoIter<Color>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a> IntoIterator for &'a Image {
	type Item = &'a Color;
	type IntoIter = std::slice::Iter<'a, Color>;
	fn into_iter(self) -> Self::IntoIter {
		self.data.iter()
	}
}

pub fn file_node<'n, P: AsRef<Path> + 'n>() -> impl Node<P, Output = Result<Vec<u8>, Error>> {
	let fs = ValueNode(StdFs).clone();
	let fs = ConsNode::new(fs);
	let file = fs.then(FileNode(PhantomData));

	file.then(FlatMapResultNode::new(BufferNode))
}

pub fn image_node<'n, P: AsRef<Path> + 'n>() -> impl Node<P, Output = Result<Image, Error>> {
	let file = file_node();
	let image_loader = FnNode::new(|data: Vec<u8>| image::load_from_memory(&data).map_err(Error::Image).map(|image| image.into_rgba32f()));
	let image: ComposeNode<_, _, P> = file.then(FlatMapResultNode::new(image_loader));
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

pub fn export_image_node<'n>() -> impl Node<(Image, &'n str), Output = Result<(), Error>> {
	FnNode::new(|input: (Image, &str)| {
		let (image, path) = input;
		let mut new_image = image::ImageBuffer::new(image.width, image.height);
		for ((x, y, pixel), color) in new_image.enumerate_pixels_mut().zip((&image).into_iter()) {
			let color: Color = *color;
			assert!(x < image.width);
			assert!(y < image.height);
			*pixel = image::Rgba(color.to_rgba8())
		}
		new_image.save(path).map_err(Error::Image)
	})
}

fn grayscale_image(mut image: Image) -> Image {
	for pixel in &mut image.data {
		let avg = (pixel.r() + pixel.g() + pixel.b()) / 3.;
		*pixel = Color::from_rgbaf32_unchecked(avg, avg, avg, pixel.a());
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct GrayscaleNode;

impl Node<Image> for GrayscaleNode {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		grayscale_image(image)
	}
}
impl Node<Image> for &GrayscaleNode {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		grayscale_image(image)
	}
}

fn invert_image(mut image: Image) -> Image {
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(1. - pixel.r(), 1. - pixel.g(), 1. - pixel.b(), pixel.a());
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct InvertRGBNode;

impl Node<Image> for InvertRGBNode {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		invert_image(image)
	}
}
impl Node<Image> for &InvertRGBNode {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		invert_image(image)
	}
}

fn shift_image_hsl(mut image: Image, hue_shift: f32, saturation_shift: f32, lightness_shift: f32) -> Image {
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
pub struct HueSaturationNode<Hue, Sat, Lit>
where
	Hue: Node<(), Output = f64>,
	Sat: Node<(), Output = f64>,
	Lit: Node<(), Output = f64>,
{
	hue: Hue,
	saturation: Sat,
	lightness: Lit,
}

impl<Hue, Sat, Lit> Node<Image> for HueSaturationNode<Hue, Sat, Lit>
where
	Hue: Node<(), Output = f64>,
	Sat: Node<(), Output = f64>,
	Lit: Node<(), Output = f64>,
{
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		shift_image_hsl(image, self.hue.eval(()) as f32, self.saturation.eval(()) as f32, self.lightness.eval(()) as f32)
	}
}
impl<Hue, Sat, Lit> Node<Image> for &HueSaturationNode<Hue, Sat, Lit>
where
	Hue: Node<(), Output = f64> + Copy,
	Sat: Node<(), Output = f64> + Copy,
	Lit: Node<(), Output = f64> + Copy,
{
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		shift_image_hsl(image, self.hue.eval(()) as f32, self.saturation.eval(()) as f32, self.lightness.eval(()) as f32)
	}
}

impl<Hue, Sat, Lit> HueSaturationNode<Hue, Sat, Lit>
where
	Hue: Node<(), Output = f64>,
	Sat: Node<(), Output = f64>,
	Lit: Node<(), Output = f64>,
{
	pub fn new(hue: Hue, saturation: Sat, lightness: Lit) -> Self {
		Self { hue, saturation, lightness }
	}
}

// Copy pasta from https://stackoverflow.com/questions/2976274/adjust-bitmap-image-brightness-contrast-using-c
fn adjust_image_brightness_and_contrast(mut image: Image, brightness_shift: f32, contrast: f32) -> Image {
	let factor = (259. * (contrast + 255.)) / (255. * (259. - contrast));
	let channel = |channel: f32| ((factor * (channel * 255. + brightness_shift - 128.) + 128.) / 255.).clamp(0., 1.);

	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct BrightnessContrastNode<Brightness, Contrast>
where
	Brightness: Node<(), Output = f64>,
	Contrast: Node<(), Output = f64>,
{
	brightness: Brightness,
	contrast: Contrast,
}

impl<Brightness, Contrast> Node<Image> for BrightnessContrastNode<Brightness, Contrast>
where
	Brightness: Node<(), Output = f64>,
	Contrast: Node<(), Output = f64>,
{
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		adjust_image_brightness_and_contrast(image, self.brightness.eval(()) as f32, self.contrast.eval(()) as f32)
	}
}

impl<Brightness, Contrast> Node<Image> for &BrightnessContrastNode<Brightness, Contrast>
where
	Brightness: Node<(), Output = f64> + Copy,
	Contrast: Node<(), Output = f64> + Copy,
{
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		adjust_image_brightness_and_contrast(image, self.brightness.eval(()) as f32, self.contrast.eval(()) as f32)
	}
}

impl<Brightness, Contrast> BrightnessContrastNode<Brightness, Contrast>
where
	Brightness: Node<(), Output = f64>,
	Contrast: Node<(), Output = f64>,
{
	pub fn new(brightness: Brightness, contrast: Contrast) -> Self {
		Self { brightness, contrast }
	}
}

// https://www.dfstudios.co.uk/articles/programming/image-programming-algorithms/image-processing-algorithms-part-6-gamma-correction/
fn image_gamma(mut image: Image, gamma: f32) -> Image {
	let inverse_gamma = 1. / gamma;
	let channel = |channel: f32| channel.powf(inverse_gamma);
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct GammaNode<N: Node<(), Output = f64>>(N);

impl<N: Node<(), Output = f64>> Node<Image> for GammaNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		image_gamma(image, self.0.eval(()) as f32)
	}
}
impl<N: Node<(), Output = f64> + Copy> Node<Image> for &GammaNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		image_gamma(image, self.0.eval(()) as f32)
	}
}

impl<N: Node<(), Output = f64> + Copy> GammaNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

fn image_opacity(mut image: Image, opacity_multiplier: f32) -> Image {
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(pixel.r(), pixel.g(), pixel.b(), pixel.a() * opacity_multiplier)
	}
	image
}

// Based on http://www.axiomx.com/posterize.htm
fn posterize(mut image: Image, posterize_value: f32) -> Image {
	let number_of_areas = posterize_value.recip();
	let size_of_areas = (posterize_value - 1.).recip();
	let channel = |channel: f32| (channel / number_of_areas).floor() * size_of_areas;
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

// Based on https://stackoverflow.com/questions/12166117/what-is-the-math-behind-exposure-adjustment-on-photoshop
fn exposure(mut image: Image, exposure: f32) -> Image {
	let multiplier = 2f32.powf(exposure);
	let channel = |channel: f32| channel * multiplier;
	for pixel in &mut image.data {
		*pixel = Color::from_rgbaf32_unchecked(channel(pixel.r()), channel(pixel.g()), channel(pixel.b()), pixel.a())
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct PosterizeNode<N: Node<(), Output = f64>>(N);

impl<N: Node<(), Output = f64>> Node<Image> for PosterizeNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		posterize(image, self.0.eval(()) as f32)
	}
}
impl<N: Node<(), Output = f64> + Copy> Node<Image> for &PosterizeNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		posterize(image, self.0.eval(()) as f32)
	}
}

impl<N: Node<(), Output = f64> + Copy> PosterizeNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct OpacityNode<N: Node<(), Output = f64>>(N);

impl<N: Node<(), Output = f64>> Node<Image> for OpacityNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		image_opacity(image, self.0.eval(()) as f32)
	}
}
impl<N: Node<(), Output = f64> + Copy> Node<Image> for &OpacityNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		image_opacity(image, self.0.eval(()) as f32)
	}
}

impl<N: Node<(), Output = f64> + Copy> OpacityNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct ExposureNode<N: Node<(), Output = f64>>(N);

impl<N: Node<(), Output = f64>> Node<Image> for ExposureNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		exposure(image, self.0.eval(()) as f32)
	}
}
impl<N: Node<(), Output = f64> + Copy> Node<Image> for &ExposureNode<N> {
	type Output = Image;
	fn eval(self, image: Image) -> Image {
		exposure(image, self.0.eval(()) as f32)
	}
}

impl<N: Node<(), Output = f64> + Copy> ExposureNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::raster::color::Color;
	use graphene_core::raster::GrayscaleColorNode;

	#[test]
	fn map_node() {
		let array = [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];
		let map = MapNode(GrayscaleColorNode, PhantomData);
		let values = map.eval(array.into_iter());
		assert_eq!(values[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());
	}

	#[test]
	fn load_image() {
		let image = image_node::<&str>();
		let gray = MapImageNode::new(GrayscaleColorNode);

		let grayscale_picture = image.then(MapResultNode::new(&gray));
		let export = export_image_node();

		let picture = grayscale_picture.eval("test-image-1.png").expect("Failed to load image");
		export.eval((picture, "test-image-1-result.png")).unwrap();
	}
}
