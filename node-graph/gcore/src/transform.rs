use crate::application_io::TextureFrame;
use crate::raster::bbox::AxisAlignedBbox;
use crate::raster::{ImageFrame, Pixel};
use crate::vector::VectorData;
use crate::{Artboard, ArtboardGroup, Color, ContextImpl, Ctx, ExtractFootprint, GraphicElement, GraphicGroup};
use crate::{Context, OwnedContextImpl};

use glam::{DAffine2, DVec2};

pub trait Transform {
	fn transform(&self) -> DAffine2;
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		pivot
	}
	fn decompose_scale(&self) -> DVec2 {
		DVec2::new(
			self.transform().transform_vector2((1., 0.).into()).length(),
			self.transform().transform_vector2((0., 1.).into()).length(),
		)
	}
}

impl<T: Transform> Transform for &T {
	fn transform(&self) -> DAffine2 {
		(*self).transform()
	}
}

pub trait TransformMut: Transform {
	fn transform_mut(&mut self) -> &mut DAffine2;
	fn translate(&mut self, offset: DVec2) {
		*self.transform_mut() = DAffine2::from_translation(offset) * self.transform();
	}
}

impl<P: Pixel> Transform for ImageFrame<P> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.local_pivot(pivot)
	}
}
impl<P: Pixel> TransformMut for ImageFrame<P> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}
impl Transform for GraphicGroup {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformMut for GraphicGroup {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}
impl Transform for GraphicElement {
	fn transform(&self) -> DAffine2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.transform(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.transform(),
			GraphicElement::Raster(raster) => raster.transform(),
		}
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.local_pivot(pivot),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.local_pivot(pivot),
			GraphicElement::Raster(raster) => raster.local_pivot(pivot),
		}
	}
}
impl TransformMut for GraphicElement {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.transform_mut(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.transform_mut(),
			GraphicElement::Raster(raster) => raster.transform_mut(),
		}
	}
}

impl Transform for VectorData {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.local_pivot(pivot)
	}
}
impl TransformMut for VectorData {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

impl Transform for Artboard {
	fn transform(&self) -> DAffine2 {
		DAffine2::from_translation(self.location.as_dvec2())
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.location.as_dvec2() + self.dimensions.as_dvec2() * pivot
	}
}

impl Transform for DAffine2 {
	fn transform(&self) -> DAffine2 {
		*self
	}
}
impl TransformMut for DAffine2 {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self
	}
}

#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderQuality {
	/// Low quality, fast rendering
	Preview,
	/// Ensure that the render is available with at least the specified quality
	/// A value of 0.5 means that the render is available with at least 50% of the final image resolution
	Scale(f32),
	/// Flip a coin to decide if the render should be available with the current quality or done at full quality
	/// This should be used to gradually update the render quality of a cached node
	Probability(f32),
	/// Render at full quality
	Full,
}
#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Footprint {
	/// Inverse of the transform which will be applied to the node output during the rendering process
	pub transform: DAffine2,
	/// Resolution of the target output area in pixels
	pub resolution: glam::UVec2,
	/// Quality of the render, this may be used by caching nodes to decide if the cached render is sufficient
	pub quality: RenderQuality,
	/// When the transform is set downstream, all upstream modifications have to be ignored
	pub ignore_modifications: bool,
}

impl Default for Footprint {
	fn default() -> Self {
		Self {
			transform: DAffine2::IDENTITY,
			resolution: glam::UVec2::new(1920, 1080),
			quality: RenderQuality::Full,
			ignore_modifications: false,
		}
	}
}

impl Footprint {
	pub fn viewport_bounds_in_local_space(&self) -> AxisAlignedBbox {
		let inverse = self.transform.inverse();
		let start = inverse.transform_point2((0., 0.).into());
		let end = inverse.transform_point2(self.resolution.as_dvec2());
		AxisAlignedBbox { start, end }
	}

	pub fn scale(&self) -> DVec2 {
		self.transform.decompose_scale()
	}

	pub fn offset(&self) -> DVec2 {
		self.transform.transform_point2(DVec2::ZERO)
	}
}

impl From<()> for Footprint {
	fn from(_: ()) -> Self {
		Footprint::default()
	}
}

#[node_macro::node(category("Debug"))]
fn cull<T>(_: impl Ctx, #[implementations(VectorData, GraphicGroup, Artboard, ImageFrame<Color>, ArtboardGroup)] data: T) -> T {
	data
}

impl core::hash::Hash for Footprint {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.transform.to_cols_array().iter().for_each(|x| x.to_le_bytes().hash(state));
		self.resolution.hash(state)
	}
}

impl Transform for Footprint {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformMut for Footprint {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

pub trait ApplyTransform {
	fn apply_transform(&mut self, modification: &DAffine2);
}
impl<T: TransformMut> ApplyTransform for T {
	fn apply_transform(&mut self, &modification: &DAffine2) {
		*self.transform_mut() = self.transform() * modification
	}
}
impl ApplyTransform for () {
	fn apply_transform(&mut self, &_modification: &DAffine2) {}
}

// async fn transform2<'call, 'input: 'call, 'n: 'call, T: 'n + TransformMut, _Input: ExtractFootprint + 'call>(
// 	input: _Input,
// 	transform_target: impl crate::Node<'call, Context<'call>, Output = T> + 'n,
// ) -> T
// where
// {
// 	let footprint = input.footprint().unwrap();
// 	let ctx: ContextImpl<'_> = ContextImpl {
// 		footprint: Some(&footprint),
// 		..Default::default()
// 	};
// 	let mut transform_target = transform_target.eval(Some(&ctx));
// 	transform_target
// }

// struct Context2<'a>(&'a str);

// async fn transform2<'call, 'n: 'call, T: 'n>(
// 	// input: _Input,
// 	transform_target: &'n (impl for<'all_input> crate::Node<'all_input, Context2<'all_input>, Output: core::future::Future<Output = T>> + Sync),
// ) -> T {
// 	// // let footprint = *input.footprint().unwrap();
// 	// let ctx: ContextImpl<'_> = ContextImpl {
// 	// 	// footprint: Some(&footprint),
// 	// 	..Default::default()
// 	// };
// 	// let transform_target = transform_target.eval(Some(&ctx)).await;
// 	// transform_target
// 	let string = String::from("test");

// 	transform_target.eval(Context2(&string.as_ref())).await
// }

// async fn transform3<'call, 'n: 'call, T: 'n>(transform_target: impl for<'all_input> crate::Node<'all_input, Context2<'all_input>, Output = impl core::future::Future<Output = T>> + Sync) -> T {
// 	let string = String::from("test");

// 	transform_target.eval(Context2(&string.as_ref())).await
// }

// // impl<'call, 'n: 'call, T: 'n, _Input: 'n, Node0> crate::Node<'n, _Input> for TransformNode<Node0>
// // where
// // 	Node0: for<'all_input> crate::Node<'all_input, Context2<'all_input>, Output: core::future::Future<Output = T>>,
// // 	// for<'a, 'b, 'c> <Node0 as crate::Node<'a, std::option::Option<&'b ContextImpl<'b>>>>::Output: crate::WasmNotSend,
// // 	Node0: Sync + Send,
// // {
// // 	type Output = core::pin::Pin<Box<dyn core::future::Future<Output = T> + 'n>>;
// // 	#[inline]
// // 	fn eval(&'n self, __input: _Input) -> Self::Output {
// // 		let transform_target = &self.transform_target;
// // 		Box::pin(self::transform3(transform_target))
// // 	}
// // }

// // impl<'call, 'n: 'call, T: 'n, _Input: 'n + ExtractFootprint, Node0> crate::Node<'n, _Input> for TransformNode<Node0>
// impl<'call, 'n: 'call, T: 'n, _Input: 'n, Node0, F0> crate::Node<'n, _Input> for TransformNode<Node0>
// where
// 	Node0: for<'all_input> crate::Node<'all_input, Context2<'all_input>, Output = F0>,
// 	F0: core::future::Future<Output = T> + Send,
// 	// for<'a, 'b, 'c> <Node0 as crate::Node<'a, std::option::Option<&'b ContextImpl<'b>>>>::Output: crate::WasmNotSend,
// 	Node0: Sync + Send,
// {
// 	// type Output = crate::registry::DynFuture<'n, T>;
// 	type Output = core::pin::Pin<Box<dyn core::future::Future<Output = T> + 'n + Send>>;
// 	#[inline]
// 	fn eval(&'n self, __input: _Input) -> Self::Output {
// 		let transform_target = &self.transform_target;
// 		Box::pin(self::transform3(transform_target))
// 	}
// }

// pub struct TransformNode<Node0> {
// 	pub(super) transform_target: Node0,
// }
#[node_macro::node(category(""))]
async fn transform<T: 'n + TransformMut + 'static>(
	input: impl ExtractFootprint + Ctx,
	#[implementations(
		Context -> VectorData,
		Context -> GraphicGroup,
		Context -> ImageFrame<Color>,
		Context -> TextureFrame,
	)]
	transform_target: impl Node<Context<'static>, Output = T>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	shear: DVec2,
	_pivot: DVec2,
) -> T
where
{
	let modification = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]);
	let mut footprint = *input.footprint().unwrap();

	if !footprint.ignore_modifications {
		footprint.apply_transform(&modification);
	}
	let ctx = OwnedContextImpl {
		footprint: Some(footprint),
		..Default::default()
	};

	let mut transform_target = transform_target.eval(Some(ctx.into())).await;

	let data_transform = transform_target.transform_mut();
	*data_transform = modification * (*data_transform);

	transform_target
}

#[node_macro::node(category(""))]
fn replace_transform<Data: TransformMut, TransformInput: Transform>(
	_: impl Ctx,
	#[implementations(VectorData, ImageFrame<Color>, GraphicGroup)] mut data: Data,
	#[implementations(DAffine2)] transform: TransformInput,
) -> Data {
	let data_transform = data.transform_mut();
	*data_transform = transform.transform();
	data
}
