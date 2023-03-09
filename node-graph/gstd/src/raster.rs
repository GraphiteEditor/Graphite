use dyn_any::{DynAny, StaticType};

use glam::{BVec2, DAffine2, DVec2};
use graphene_core::raster::{Color, Image, ImageFrame};
use graphene_core::Node;

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
	fs.open(path)
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
		for ((x, y, pixel), color) in new_image.enumerate_pixels_mut().zip(image.data.iter()) {
			let color: Color = *color;
			assert!(x < image.width);
			assert!(y < image.height);
			*pixel = image::Rgba(color.to_rgba8())
		}
		new_image.save(path).map_err(Error::Image)
	})
}
*/

#[derive(Debug, Clone, Copy)]
pub struct MapImageNode<MapFn> {
	map_fn: MapFn,
}

#[node_macro::node_fn(MapImageNode)]
fn map_image<MapFn>(image: Image, map_fn: &'any_input MapFn) -> Image
where
	MapFn: for<'any_input> Node<'any_input, Color, Output = Color> + 'input,
{
	let mut image = image;
	for pixel in &mut image.data {
		*pixel = map_fn.eval(*pixel);
	}
	image
}

#[derive(Debug, Clone, Copy)]
pub struct MapImageFrameNode<MapFn> {
	map_fn: MapFn,
}

#[node_macro::node_fn(MapImageFrameNode)]
fn map_image<MapFn>(mut image_frame: ImageFrame, map_fn: &'any_input MapFn) -> ImageFrame
where
	MapFn: for<'any_input> Node<'any_input, Color, Output = Color> + 'input,
{
	for pixel in &mut image_frame.image.data {
		*pixel = map_fn.eval(*pixel);
	}

	image_frame
}

#[derive(Debug, Clone)]
struct AxisAlignedBbox {
	start: DVec2,
	end: DVec2,
}

#[derive(Debug, Clone)]
struct Bbox {
	top_left: DVec2,
	top_right: DVec2,
	bottom_left: DVec2,
	bottom_right: DVec2,
}

impl Bbox {
	fn axis_aligned_bbox(&self) -> AxisAlignedBbox {
		let start_x = self.top_left.x.min(self.top_right.x).min(self.bottom_left.x).min(self.bottom_right.x);
		let start_y = self.top_left.y.min(self.top_right.y).min(self.bottom_left.y).min(self.bottom_right.y);
		let end_x = self.top_left.x.max(self.top_right.x).max(self.bottom_left.x).max(self.bottom_right.x);
		let end_y = self.top_left.y.max(self.top_right.y).max(self.bottom_left.y).max(self.bottom_right.y);

		AxisAlignedBbox {
			start: DVec2::new(start_x, start_y),
			end: DVec2::new(end_x, end_y),
		}
	}
}

fn compute_transformed_bounding_box(transform: DAffine2) -> Bbox {
	let top_left = DVec2::new(0., 1.);
	let top_right = DVec2::new(1., 1.);
	let bottom_left = DVec2::new(0., 0.);
	let bottom_right = DVec2::new(1., 0.);
	let transform = |p| transform.transform_point2(p);

	Bbox {
		top_left: transform(top_left),
		top_right: transform(top_right),
		bottom_left: transform(bottom_left),
		bottom_right: transform(bottom_right),
	}
}

#[derive(Debug, Clone, Copy)]
pub struct BlendImageNode<Destination, MapFn> {
	destination: Destination,
	map_fn: MapFn,
}

// TODO: Implement proper blending
#[node_macro::node_fn(BlendImageNode)]
fn blend_image<MapFn>(source: ImageFrame, mut destination: ImageFrame, map_fn: &'any_input MapFn) -> ImageFrame
where
	MapFn: for<'any_input> Node<'any_input, (Color, Color), Output = Color> + 'input,
{
	// Transforms a point from the source image to the destination image
	let dest_to_src = source.transform * destination.transform.inverse();

	let source_size = DVec2::new(source.image.width as f64, source.image.height as f64);
	let destination_size = DVec2::new(destination.image.width as f64, destination.image.height as f64);

	// Footprint of the source image (0,0) (1, 1) in the destination image space
	let src_aabb = compute_transformed_bounding_box(destination.transform.inverse() * source.transform).axis_aligned_bbox();

	// Clamp the source image to the destination image
	let start = (src_aabb.start * destination_size).max(DVec2::ZERO).as_ivec2();
	let end = (src_aabb.end * destination_size).min(destination_size).as_ivec2();

	log::info!("start: {:?}, end: {:?}", start, end);
	log::info!("src_aabb: {:?}", src_aabb);
	log::info!("destination_size: {:?}", destination_size);
	log::info!("src_to_dst: {:?}", dest_to_src);
	log::info!("source.transform: {:?}", source.transform);
	log::info!("destination.transform: {:?}", destination.transform);

	for y in start.y..end.y {
		for x in start.x..end.x {
			let dest_uv = DVec2::new(x as f64, y as f64);
			let src_uv = dest_to_src.transform_point2(dest_uv);
			if !((src_uv.cmpge(DVec2::ZERO) & src_uv.cmple(source_size)) == BVec2::new(true, true)) {
				//log::debug!("Skipping pixel at {:?}", dest_point);
				continue;
			}

			let source_point = src_uv;

			let dst_pixel = destination.get_mut(x as usize, y as usize);
			let src_pixel = source.sample(source_point.x, source_point.y);

			*dst_pixel = map_fn.eval((src_pixel, *dst_pixel));
		}
	}

	destination
}

#[derive(Debug, Clone, Copy)]
pub struct ImaginateNode<E> {
	cached: E,
}

#[node_macro::node_fn(ImaginateNode)]
fn imaginate(image_frame: ImageFrame, cached: Option<std::sync::Arc<graphene_core::raster::Image>>) -> ImageFrame {
	info!("Imaginating image with {} pixels", image_frame.image.data.len());
	let cached_image = cached.map(|mut x| std::sync::Arc::make_mut(&mut x).clone()).unwrap_or(image_frame.image);
	ImageFrame {
		image: cached_image,
		transform: image_frame.transform,
	}
}

#[derive(Debug, Clone, Copy)]
pub struct ImageFrameNode<Transform> {
	transform: Transform,
}
#[node_macro::node_fn(ImageFrameNode)]
fn image_frame(image: Image, transform: DAffine2) -> graphene_core::raster::ImageFrame {
	graphene_core::raster::ImageFrame { image, transform }
}
#[cfg(test)]
mod test {

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
