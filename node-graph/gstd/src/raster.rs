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

#[derive(Debug, Clone, Copy)]
pub struct GrayscaleImageNode;

impl Node<Image> for GrayscaleImageNode {
	type Output = Image;
	fn eval(self, mut image: Image) -> Image {
		for pixel in &mut image.data {
			let avg = (pixel.r() + pixel.g() + pixel.b()) / 3.;
			*pixel = Color::from_rgbaf32_unchecked(avg, avg, avg, pixel.a());
		}
		image
	}
}
impl Node<Image> for &GrayscaleImageNode {
	type Output = Image;
	fn eval(self, mut image: Image) -> Image {
		for pixel in &mut image.data {
			let avg = (pixel.r() + pixel.g() + pixel.b()) / 3.;
			*pixel = Color::from_rgbaf32_unchecked(avg, avg, avg, pixel.a());
		}
		image
	}
}

#[derive(Debug, Clone, Copy)]
pub struct BrightenImageNode<N: Node<(), Output = f32>>(N);

impl<N: Node<(), Output = f32>> Node<Image> for BrightenImageNode<N> {
	type Output = Image;
	fn eval(self, mut image: Image) -> Image {
		let brightness = self.0.eval(());
		let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
		for pixel in &mut image.data {
			*pixel = Color::from_rgbaf32_unchecked(per_channel(pixel.r()), per_channel(pixel.g()), per_channel(pixel.b()), pixel.a());
		}
		image
	}
}
impl<N: Node<(), Output = f32> + Copy> Node<Image> for &BrightenImageNode<N> {
	type Output = Image;
	fn eval(self, mut image: Image) -> Image {
		let brightness = self.0.eval(());
		let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
		for pixel in &mut image.data {
			*pixel = Color::from_rgbaf32_unchecked(per_channel(pixel.r()), per_channel(pixel.g()), per_channel(pixel.b()), pixel.a());
		}
		image
	}
}

impl<N: Node<(), Output = f32> + Copy> BrightenImageNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct HueShiftImage<N: Node<(), Output = f32>>(N);

impl<N: Node<(), Output = f32>> Node<Image> for HueShiftImage<N> {
	type Output = Image;
	fn eval(self, mut image: Image) -> Image {
		let hue_shift = self.0.eval(());
		for pixel in &mut image.data {
			let [hue, saturation, luminance, alpha] = pixel.to_hsla();
			*pixel = Color::from_hsla(hue + hue_shift / 360., saturation, luminance, alpha);
		}
		image
	}
}
impl<N: Node<(), Output = f32> + Copy> Node<Image> for &HueShiftImage<N> {
	type Output = Image;
	fn eval(self, mut image: Image) -> Image {
		let hue_shift = self.0.eval(());
		for pixel in &mut image.data {
			let [hue, saturation, luminance, alpha] = pixel.to_hsla();
			*pixel = Color::from_hsla(hue + hue_shift / 360., saturation, luminance, alpha);
		}
		image
	}
}

impl<N: Node<(), Output = f32> + Copy> HueShiftImage<N> {
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
