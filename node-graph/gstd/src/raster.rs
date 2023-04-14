use dyn_any::{DynAny, StaticType, StaticTypeSized};

use glam::{BVec2, DAffine2, DVec2};
use graphene_core::raster::{Color, Image, ImageFrame};
use graphene_core::transform::Transform;
use graphene_core::value::{ClonedNode, ValueNode};
use graphene_core::Node;

use std::marker::PhantomData;
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

pub struct DownresNode;

#[node_macro::node_fn(DownresNode)]
fn downres(image_frame: ImageFrame) -> ImageFrame {
	let target_width = (image_frame.transform.transform_vector2((1., 0.).into()).length() as usize).min(image_frame.image.width as usize);
	let target_height = (image_frame.transform.transform_vector2((0., 1.).into()).length() as usize).min(image_frame.image.height as usize);

	let mut image = Image {
		width: target_width as u32,
		height: target_height as u32,
		data: Vec::with_capacity(target_width * target_height),
	};

	let scale_factor = DVec2::new(image_frame.image.width as f64, image_frame.image.height as f64) / DVec2::new(target_width as f64, target_height as f64);
	for y in 0..target_height {
		for x in 0..target_width {
			let pixel = image_frame.sample(DVec2::new(x as f64, y as f64) * scale_factor);
			image.data.push(pixel);
		}
	}

	ImageFrame {
		image,
		transform: image_frame.transform,
	}
}

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

impl<MapFn: dyn_any::StaticTypeSized> StaticType for MapImageFrameNode<MapFn> {
	type Static = MapImageFrameNode<MapFn::Static>;
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

#[derive(Debug, Clone, DynAny)]
pub struct AxisAlignedBbox {
	start: DVec2,
	end: DVec2,
}

impl AxisAlignedBbox {
	pub fn size(&self) -> DVec2 {
		self.end - self.start
	}

	pub fn to_transform(&self) -> DAffine2 {
		DAffine2::from_translation(self.start) * DAffine2::from_scale(self.size())
	}

	pub fn contains(&self, point: DVec2) -> bool {
		point.x >= self.start.x && point.x <= self.end.x && point.y >= self.start.y && point.y <= self.end.y
	}

	pub fn intersects(&self, other: &AxisAlignedBbox) -> bool {
		other.start.x <= self.end.x && other.end.x >= self.start.x && other.start.y <= self.end.y && other.end.y >= self.start.y
	}

	pub fn union(&self, other: &AxisAlignedBbox) -> AxisAlignedBbox {
		AxisAlignedBbox {
			start: DVec2::new(self.start.x.min(other.start.x), self.start.y.min(other.start.y)),
			end: DVec2::new(self.end.x.max(other.end.x), self.end.y.max(other.end.y)),
		}
	}
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
pub struct MaskImageNode<Mask> {
	mask: Mask,
}

#[node_macro::node_fn(MaskImageNode)]
fn mask_image(mut image: ImageFrame, mask: ImageFrame) -> ImageFrame {
	let image_size = DVec2::new(image.image.width as f64, image.image.height as f64);
	let mask_size = DVec2::new(mask.image.width as f64, mask.image.height as f64);

	if mask_size == DVec2::ZERO {
		return image;
	}

	// Transforms a point from the background image to the forground image
	let bg_to_fg = DAffine2::from_scale(mask_size) * mask.transform.inverse() * image.transform * DAffine2::from_scale(1. / image_size);

	for y in 0..image.image.height {
		for x in 0..image.image.width {
			let image_point = DVec2::new(x as f64, y as f64);
			let mut mask_point = bg_to_fg.transform_point2(image_point);
			mask_point = mask_point.clamp(DVec2::ZERO, mask_size);

			let image_pixel = image.get_mut(x as usize, y as usize);
			let mask_pixel = mask.sample(mask_point);
			let alpha = image_pixel.a() * mask_pixel.r();

			*image_pixel = Color::from_rgbaf32(image_pixel.r(), image_pixel.g(), image_pixel.b(), alpha).unwrap();
		}
	}

	image
}

#[derive(Debug, Clone, Copy)]
pub struct BlendImageTupleNode<MapFn> {
	map_fn: MapFn,
}

impl<MapFn: StaticTypeSized> StaticType for BlendImageTupleNode<MapFn> {
	type Static = BlendImageTupleNode<MapFn::Static>;
}

#[node_macro::node_fn(BlendImageTupleNode)]
fn blend_image_tuple<MapFn>(images: (ImageFrame, ImageFrame), map_fn: &'any_input MapFn) -> ImageFrame
where
	MapFn: for<'any_input> Node<'any_input, (Color, Color), Output = Color> + 'input + Clone,
{
	let (background, foreground) = images;

	let node = BlendImageNode::new(ClonedNode::new(background), ValueNode::new(map_fn.clone()));
	node.eval(foreground)
}

#[derive(Debug, Clone, Copy)]
pub struct BlendImageNode<Background, MapFn> {
	background: Background,
	map_fn: MapFn,
}

impl<Background: StaticTypeSized, MapFn: StaticTypeSized> StaticType for BlendImageNode<Background, MapFn> {
	type Static = BlendImageNode<Background::Static, MapFn::Static>;
}

// TODO: Implement proper blending
#[node_macro::node_fn(BlendImageNode)]
fn blend_image<MapFn, Frame: AsRef<ImageFrame>>(foreground: Frame, mut background: ImageFrame, map_fn: &'any_input MapFn) -> ImageFrame
where
	MapFn: for<'any_input> Node<'any_input, (Color, Color), Output = Color> + 'input,
{
	let foreground = foreground.as_ref();
	let foreground_size = DVec2::new(foreground.image.width as f64, foreground.image.height as f64);
	let background_size = DVec2::new(background.image.width as f64, background.image.height as f64);

	// Transforms a point from the background image to the forground image
	let bg_to_fg = DAffine2::from_scale(foreground_size) * foreground.transform.inverse() * background.transform * DAffine2::from_scale(1. / background_size);

	// Footprint of the foreground image (0,0) (1, 1) in the background image space
	let bg_aabb = compute_transformed_bounding_box(background.transform.inverse() * foreground.transform).axis_aligned_bbox();

	// Clamp the foreground image to the background image
	let start = (bg_aabb.start * background_size).max(DVec2::ZERO).as_uvec2();
	let end = (bg_aabb.end * background_size).min(background_size).as_uvec2();

	for y in start.y..end.y {
		for x in start.x..end.x {
			let bg_point = DVec2::new(x as f64, y as f64);
			let fg_point = bg_to_fg.transform_point2(bg_point);
			if !((fg_point.cmpge(DVec2::ZERO) & fg_point.cmple(foreground_size)) == BVec2::new(true, true)) {
				continue;
			}

			let dst_pixel = background.get_mut(x as usize, y as usize);
			let src_pixel = foreground.sample(fg_point);

			*dst_pixel = map_fn.eval((src_pixel, *dst_pixel));
		}
	}

	background
}

#[derive(Clone, Debug, PartialEq)]
pub struct MergeBoundingBoxNode<Data> {
	_data: PhantomData<Data>,
}

#[node_macro::node_fn(MergeBoundingBoxNode<_Data>)]
fn merge_bounding_box_node<_Data: Transform>(input: (Option<AxisAlignedBbox>, _Data)) -> Option<AxisAlignedBbox> {
	let (initial_aabb, data) = input;

	let snd_aabb = compute_transformed_bounding_box(data.transform()).axis_aligned_bbox();

	if let Some(fst_aabb) = initial_aabb {
		Some(fst_aabb.union(&snd_aabb))
	} else {
		Some(snd_aabb)
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmptyImageNode<FillColor> {
	pub color: FillColor,
}

#[node_macro::node_fn(EmptyImageNode)]
fn empty_image(transform: DAffine2, color: Color) -> ImageFrame {
	let width = transform.transform_vector2(DVec2::new(1., 0.)).length() as u32;
	let height = transform.transform_vector2(DVec2::new(0., 1.)).length() as u32;

	let image = Image::new(width, height, color);
	ImageFrame { image, transform }
}

#[derive(Debug, Clone, Copy)]
pub struct ImaginateNode<E> {
	cached: E,
}

#[node_macro::node_fn(ImaginateNode)]
fn imaginate(image_frame: ImageFrame, cached: Option<std::sync::Arc<graphene_core::raster::Image>>) -> ImageFrame {
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
