use core::marker::PhantomData;
use graphene_core::ops::FlatMapResultNode;
use graphene_core::raster::color::Color;
use graphene_core::structural::{ComposeNode, ConsNode};
use graphene_core::{generic::FnNode, ops::MapResultNode, structural::After, value::ValueNode, Node};
use image::Pixel;
use std::path::Path;

pub struct MapNode<'n, MN: Node<'n, S>, I: IntoIterator<Item = S>, S>(pub MN, PhantomData<&'n (S, I)>);

impl<'n, I: IntoIterator<Item = S>, MN: Node<'n, S>, S> Node<'n, I> for MapNode<'n, MN, I, S> {
	type Output = Vec<MN::Output>;
	fn eval(&'n self, input: I) -> Self::Output {
		input.into_iter().map(|x| self.0.eval(x)).collect()
	}
}

impl<'n, I: IntoIterator<Item = S>, MN: Node<'n, S>, S> MapNode<'n, MN, I, S> {
	pub const fn new(mn: MN) -> Self {
		MapNode(mn, PhantomData)
	}
}

pub struct MapImageNode<'n, MN: Node<'n, Color, Output = Color>>(pub MN, PhantomData<&'n ()>);

impl<'n, MN: Node<'n, Color, Output = Color>> Node<'n, Image> for MapImageNode<'n, MN> {
	type Output = Image;
	fn eval(&'n self, input: Image) -> Self::Output {
		Image {
			width: input.width,
			height: input.height,
			data: input.data.iter().map(|x| self.0.eval(*x)).collect(),
		}
	}
}

impl<'n, MN: Node<'n, Color, Output = Color>> MapImageNode<'n, MN> {
	pub const fn new(mn: MN) -> Self {
		MapImageNode(mn, PhantomData)
	}
}

#[derive(Debug)]
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
impl<'n, P: AsRef<Path>, FS: FileSystem> Node<'n, (P, FS)> for FileNode<P, FS> {
	type Output = Result<Reader, Error>;

	fn eval(&'n self, input: (P, FS)) -> Self::Output {
		let (path, fs) = input;
		fs.open(path)
	}
}

pub struct BufferNode;
impl<'n, Reader: std::io::Read> Node<'n, Reader> for BufferNode {
	type Output = Result<Vec<u8>, Error>;

	fn eval(&'n self, mut reader: Reader) -> Self::Output {
		let mut buffer = Vec::new();
		reader.read_to_end(&mut buffer)?;
		Ok(buffer)
	}
}

#[derive(Clone)]
pub struct Image {
	pub width: u32,
	pub height: u32,
	pub data: Vec<Color>,
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

pub fn file_node<'n, P: AsRef<Path> + 'n>() -> impl Node<'n, P, Output = Result<Vec<u8>, Error>> {
	let fs = ValueNode(StdFs).clone();
	let fs = ConsNode(fs);
	let file: ComposeNode<P, _, FileNode<P, _>> = FileNode(PhantomData).after(fs);
	let buffer = FlatMapResultNode::new(BufferNode).after(file);
	buffer
}
type Ret<'n> = impl Node<'n, (), Output = u32>;

pub fn test_node<'n>() -> Ret<'n> {
	ValueNode(432).clone()
}

pub fn image_node<'n, P: AsRef<Path> + 'n>() -> impl Node<'n, P, Output = Result<Image, Error>> {
	let file = file_node();
	let image_loader = FnNode::new(|data: Vec<u8>| image::load_from_memory(&data).map_err(Error::Image).map(|image| image.into_rgba32f()));
	let image: ComposeNode<'_, P, _, _> = FlatMapResultNode::new(image_loader).after(file);
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
	let image = MapResultNode::new(convert_image).after(image);
	image
}

pub fn export_image_node<'n>() -> impl Node<'n, (Image, &'n str), Output = Result<(), Error>> {
	FnNode::new(|input: (Image, &str)| {
		let (image, path) = input;
		let mut new_image = image::ImageBuffer::new(image.width, image.height);
		for ((x, y, pixel), color) in new_image.enumerate_pixels_mut().zip((&image).into_iter()) {
			let color: Color = *color;
			assert!(x < image.width);
			assert!(y < image.height);
			*pixel = image::Rgba([color.r(), color.g(), color.b(), color.a()])
		}
		new_image.save(path).map_err(Error::Image)
	})
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::raster::color::Color;
	use graphene_core::raster::GrayscaleNode;

	#[test]
	fn map_node() {
		let array = [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];
		let map = MapNode(GrayscaleNode, PhantomData);
		let values = map.eval(array.into_iter());
		assert_eq!(values[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());
	}

	#[test]
	fn load_image() {
		/*let image = image_node();
				let gray = MapImageNode::new(GrayscaleNode);

				let gray_scale_picture = MapResultNode::new(gray).after(image);
				let gray_scale_picture = gray_scale_picture.eval("image");
		*/
		let test_node = test_node();
		{
			let foo = test_node.eval(());
			std::mem::drop(foo);
		}
		let export = export_image_node();

		/*let export = FnNode::new(|input: (&str, &str)| {
		let (input, output) = input;*/
		//let picture = gray_scale_picture.eval("/home/dennis/screenshot.png").unwrap().clone();
		//export.eval((picture, "screenshot.png"));
		/*});
		export.eval(("screenshot.png", "/home/dennis/screenshot.png"));*/
	}
}
