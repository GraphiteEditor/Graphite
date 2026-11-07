use crate::scene_builder::{SHAPE_TOLERANCE, SceneBuilder, finite_path};
use graphic_types::raster_types::{CPU, Raster, Texture};
use graphic_types::vector_types::vector::PlacedContours;
use kurbo::{Affine, BezPath, Rect, Shape, Size, Stroke};
use std::collections::HashMap;
use vello_gpu::peniko::{self, BlendMode, BrushRef, Fill, ImageQuality};

/// Records the drawing of one PDF page, which [`PdfScene::encode_pdf`] writes out once any GPU textures it draws are read back.
#[derive(Debug)]
pub struct PdfSceneBuilder {
	/// Places the drawn content on the page, applied on top of every transform it's drawn with.
	view: Affine,
	/// The page's width and height, in the pixels `view` places content into.
	size: Size,
	/// The page's own drawings, beneath the layers and clips begun and not yet ended.
	drawings: Vec<PdfDrawing>,
	/// Each layer or clip begun and not yet ended, innermost last, with the drawings made inside it so far.
	open_groups: Vec<(PdfGroupKind, Vec<PdfDrawing>)>,
	images: Vec<PdfImage>,
	image_indices: HashMap<ImageKey, usize>,
}

/// The drawing of one PDF page in the page's pixels, ready to encode as a PDF once [`Self::textures`] are read back with [`Self::resolve_texture`].
#[derive(Clone, Debug, PartialEq)]
pub struct PdfScene {
	pub(crate) size: Size,
	pub(crate) drawings: Vec<PdfDrawing>,
	/// The images drawn, each referred to by its index.
	pub(crate) images: Vec<PdfImage>,
}

/// One drawing operation on a page, with paths and transforms in the page's pixels unless noted otherwise.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PdfDrawing {
	Fill {
		path: BezPath,
		fill_rule: Fill,
		paint: PdfPaint,
	},
	/// A path stroked in its own space, which `transform` places onto the page, since a stroke's width scales with its transform.
	Stroke {
		path: BezPath,
		transform: Affine,
		stroke: Stroke,
		paint: PdfPaint,
	},
	/// The rectangle of an image's pixels, from the origin to its width and height, placed by `transform`.
	Image {
		image: usize,
		quality: ImageQuality,
		alpha: f32,
		transform: Affine,
	},
	Group {
		kind: PdfGroupKind,
		drawings: Vec<PdfDrawing>,
	},
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PdfGroupKind {
	/// An isolated layer composited onto what's beneath it, clipped to a path unless it covers the whole page.
	Layer { blend_mode: BlendMode, alpha: f32, clip: Option<(BezPath, Fill)> },
	/// A clip of the drawings within, which still composite with what's beneath them individually.
	Clip { path: BezPath, fill_rule: Fill },
}

/// What a fill or stroke paints with, placed by `transform` into the space of the drawing's path.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PdfPaint {
	Solid(peniko::Color),
	Gradient { gradient: peniko::Gradient, transform: Affine },
	Image { image: usize, sampler: peniko::ImageSampler, transform: Affine },
}

/// The pixels behind an image a page draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PdfImage {
	/// RGBA8 pixels in CPU memory.
	Pixels(peniko::ImageData),
	/// A CPU raster's straight-alpha linear pixels.
	Raster(Raster<CPU>),
	/// Straight-alpha pixels in a GPU texture, which must be read back before the page is encoded.
	Texture(Texture),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum ImageKey {
	Blob(u64),
	Raster(u64),
	Texture(Texture),
}

impl PdfSceneBuilder {
	/// A page of `size` pixels, which `view` places the drawn content onto.
	pub fn new(size: Size, view: Affine) -> Self {
		Self {
			view,
			size,
			drawings: Vec::new(),
			open_groups: Vec::new(),
			images: Vec::new(),
			image_indices: HashMap::new(),
		}
	}

	/// The finished page, closing whatever layers and clips were left open.
	pub fn finish(mut self) -> PdfScene {
		if !self.open_groups.is_empty() {
			log::error!("A PDF page leaves {} layers or clips unended", self.open_groups.len());
		}
		while !self.open_groups.is_empty() {
			self.pop_layer();
		}

		PdfScene {
			size: self.size,
			drawings: self.drawings,
			images: self.images,
		}
	}

	/// Where the next drawing goes: into the innermost open layer or clip, or onto the page itself.
	fn current_drawings(&mut self) -> &mut Vec<PdfDrawing> {
		match self.open_groups.last_mut() {
			Some((_, drawings)) => drawings,
			None => &mut self.drawings,
		}
	}

	/// A path given in the space of `transform`, placed onto the page.
	fn placed_path(&self, transform: Affine, path: &BezPath) -> BezPath {
		let mut path = finite_path(path).into_owned();
		path.apply_affine(self.view * transform);
		path
	}

	/// The paint for a brush, placed by `brush_transform` within the space that `transform` maps from.
	fn paint(&mut self, brush: BrushRef<'_>, transform: Affine, brush_transform: Option<Affine>) -> PdfPaint {
		let transform = transform * brush_transform.unwrap_or(Affine::IDENTITY);
		match brush {
			BrushRef::Solid(color) => PdfPaint::Solid(color),
			BrushRef::Gradient(gradient) => PdfPaint::Gradient {
				gradient: gradient.clone(),
				transform,
			},
			BrushRef::Image(image) => PdfPaint::Image {
				image: self.image_index(PdfImage::Pixels(image.image.clone())),
				sampler: image.sampler,
				transform,
			},
		}
	}

	/// The index of an image among those the page draws, adding it the first time it's drawn.
	fn image_index(&mut self, image: PdfImage) -> usize {
		let key = match &image {
			PdfImage::Pixels(data) => ImageKey::Blob(data.data.id()),
			PdfImage::Raster(raster) => ImageKey::Raster(raster.buffer_id()),
			PdfImage::Texture(texture) => ImageKey::Texture(texture.clone()),
		};
		*self.image_indices.entry(key).or_insert_with(|| {
			self.images.push(image);
			self.images.len() - 1
		})
	}

	fn draw_image_index(&mut self, image: usize, sampler: peniko::ImageSampler, transform: Affine) {
		let transform = self.view * transform;
		self.current_drawings().push(PdfDrawing::Image {
			image,
			quality: sampler.quality,
			alpha: sampler.alpha,
			transform,
		});
	}

	/// Records a stroke of a path given in the space of `transform`, keeping the path in that space since a stroke's width scales with it.
	fn push_stroke(&mut self, path: &BezPath, stroke: &Stroke, transform: Affine, brush: BrushRef<'_>, brush_transform: Option<Affine>) {
		let path = finite_path(path).into_owned();
		let paint = self.paint(brush, Affine::IDENTITY, brush_transform);
		let transform = self.view * transform;
		let stroke = stroke.clone();
		self.current_drawings().push(PdfDrawing::Stroke { path, transform, stroke, paint });
	}

	fn push_group(&mut self, kind: PdfGroupKind) {
		self.open_groups.push((kind, Vec::new()));
	}
}

impl SceneBuilder for PdfSceneBuilder {
	fn view(&self) -> Affine {
		self.view
	}

	fn size(&self) -> Size {
		self.size
	}

	fn fill_path<'b>(&mut self, fill_rule: Fill, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, path: &BezPath) {
		let path = self.placed_path(transform, path);
		let paint = self.paint(brush.into(), self.view * transform, brush_transform);
		self.current_drawings().push(PdfDrawing::Fill { path, fill_rule, paint });
	}

	fn stroke<'b>(&mut self, stroke: &Stroke, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, shape: &impl Shape) {
		self.push_stroke(&shape.to_path(SHAPE_TOLERANCE), stroke, transform, brush.into(), brush_transform);
	}

	fn stroke_contours<'b>(&mut self, stroke: &Stroke, transform: Affine, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>, contours: &PlacedContours) {
		self.push_stroke(contours.path(), stroke, transform, brush.into(), brush_transform);
	}

	fn fill_viewport<'b>(&mut self, brush: impl Into<BrushRef<'b>>, brush_transform: Option<Affine>) {
		let path = Rect::from_origin_size((0., 0.), self.size).to_path(SHAPE_TOLERANCE);
		let paint = self.paint(brush.into(), self.view, brush_transform);
		self.current_drawings().push(PdfDrawing::Fill {
			path,
			fill_rule: Fill::NonZero,
			paint,
		});
	}

	fn push_layer(&mut self, fill_rule: Fill, blend_mode: impl Into<BlendMode>, alpha: f32, transform: Affine, clip: &impl Shape) {
		let clip = self.placed_path(transform, &clip.to_path(SHAPE_TOLERANCE));
		self.push_group(PdfGroupKind::Layer {
			blend_mode: blend_mode.into(),
			alpha,
			clip: Some((clip, fill_rule)),
		});
	}

	fn push_viewport_layer(&mut self, blend_mode: impl Into<BlendMode>, alpha: f32) {
		self.push_group(PdfGroupKind::Layer {
			blend_mode: blend_mode.into(),
			alpha,
			clip: None,
		});
	}

	fn push_clip_layer(&mut self, fill_rule: Fill, transform: Affine, clip: &impl Shape) {
		let path = self.placed_path(transform, &clip.to_path(SHAPE_TOLERANCE));
		self.push_group(PdfGroupKind::Clip { path, fill_rule });
	}

	fn pop_layer(&mut self) {
		let Some((kind, drawings)) = self.open_groups.pop() else {
			log::error!("A PDF page ends a layer or clip it never began");
			return;
		};
		self.current_drawings().push(PdfDrawing::Group { kind, drawings });
	}

	fn draw_image(&mut self, image: &peniko::ImageBrush, transform: Affine) {
		if image.image.width == 0 || image.image.height == 0 {
			return;
		}
		let index = self.image_index(PdfImage::Pixels(image.image.clone()));
		self.draw_image_index(index, image.sampler, transform);
	}

	fn draw_raster(&mut self, raster: Raster<CPU>, sampler: peniko::ImageSampler, transform: Affine) {
		if raster.width == 0 || raster.height == 0 {
			return;
		}
		let index = self.image_index(PdfImage::Raster(raster));
		self.draw_image_index(index, sampler, transform);
	}

	fn draw_texture(&mut self, texture: Texture, sampler: peniko::ImageSampler, transform: Affine) {
		if texture.width() == 0 || texture.height() == 0 {
			return;
		}
		let index = self.image_index(PdfImage::Texture(texture));
		self.draw_image_index(index, sampler, transform);
	}
}

impl PdfScene {
	/// The GPU textures the page draws, by the image index [`Self::resolve_texture`] takes, which must be read back before encoding.
	pub fn textures(&self) -> Vec<(usize, Texture)> {
		self.images
			.iter()
			.enumerate()
			.filter_map(|(index, image)| match image {
				PdfImage::Texture(texture) => Some((index, texture.clone())),
				_ => None,
			})
			.collect()
	}

	/// Replaces a GPU texture the page draws with its pixels read back into CPU memory.
	pub fn resolve_texture(&mut self, index: usize, raster: Raster<CPU>) {
		match self.images.get_mut(index) {
			Some(image @ PdfImage::Texture(_)) => *image = PdfImage::Raster(raster),
			_ => log::error!("A PDF page has no texture at image index {index} to resolve"),
		}
	}
}
