use crate::application_io::{TextureFrame, TextureFrameTable};
use crate::raster::image::{ImageFrame, ImageFrameTable};
use crate::raster::Pixel;
use crate::transform::{Transform, TransformMut};
use crate::vector::{InstanceId, VectorData, VectorDataTable};
use crate::{AlphaBlending, GraphicElement, GraphicGroup, GraphicGroupTable, RasterFrame};

use dyn_any::StaticType;
use glam::{DAffine2, DVec2};
use std::hash::Hash;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Instances<T>
where
	T: Into<GraphicElement> + StaticType + 'static,
{
	id: Vec<InstanceId>,
	#[serde(alias = "instances")]
	instance: Vec<T>,
	#[serde(default = "one_daffine2_default")]
	transform: Vec<DAffine2>,
	#[serde(default = "one_alpha_blending_default")]
	alpha_blending: Vec<AlphaBlending>,
}

impl<T: Into<GraphicElement> + StaticType + 'static> Instances<T> {
	pub fn new(instance: T) -> Self {
		Self {
			id: vec![InstanceId::generate()],
			instance: vec![instance],
			transform: vec![DAffine2::IDENTITY],
			alpha_blending: vec![AlphaBlending::default()],
		}
	}

	pub fn one_instance(&self) -> Instance<T> {
		Instance {
			id: self.id.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			instance: self.instance.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			transform: self.transform.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			alpha_blending: self.alpha_blending.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
		}
	}

	pub fn one_instance_mut(&mut self) -> InstanceMut<T> {
		let length = self.instance.len();

		InstanceMut {
			id: self.id.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
			instance: self.instance.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
			transform: self.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
			alpha_blending: self.alpha_blending.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
		}
	}

	pub fn instances(&self) -> impl Iterator<Item = Instance<T>> {
		assert!(self.instance.len() == 1, "ONE INSTANCE EXPECTED, FOUND {} (instances)", self.instance.len());
		self.id
			.iter()
			.zip(self.instance.iter())
			.zip(self.transform.iter())
			.zip(self.alpha_blending.iter())
			.map(|(((id, instance), transform), alpha_blending)| Instance {
				id,
				instance,
				transform,
				alpha_blending,
			})
	}

	pub fn instances_mut(&mut self) -> impl Iterator<Item = InstanceMut<T>> {
		assert!(self.instance.len() == 1, "ONE INSTANCE EXPECTED, FOUND {} (instances_mut)", self.instance.len());
		self.id
			.iter_mut()
			.zip(self.instance.iter_mut())
			.zip(self.transform.iter_mut())
			.zip(self.alpha_blending.iter_mut())
			.map(|(((id, instance), transform), alpha_blending)| InstanceMut {
				id,
				instance,
				transform,
				alpha_blending,
			})
	}
}

impl<T: Into<GraphicElement> + Default + Hash + StaticType + 'static> Default for Instances<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T: Into<GraphicElement> + Hash + StaticType + 'static> core::hash::Hash for Instances<T> {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
		for instance in &self.instance {
			instance.hash(state);
		}
	}
}

impl<T: Into<GraphicElement> + PartialEq + StaticType + 'static> PartialEq for Instances<T> {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id && self.instance.len() == other.instance.len() && { self.instance.iter().zip(other.instance.iter()).all(|(a, b)| a == b) }
	}
}

unsafe impl<T: Into<GraphicElement> + StaticType + 'static> dyn_any::StaticType for Instances<T> {
	type Static = Instances<T>;
}

fn one_daffine2_default() -> Vec<DAffine2> {
	vec![DAffine2::IDENTITY]
}
fn one_alpha_blending_default() -> Vec<AlphaBlending> {
	vec![AlphaBlending::default()]
}

#[derive(Copy, Clone, Debug)]
pub struct Instance<'a, T> {
	pub id: &'a InstanceId,
	pub instance: &'a T,
	pub transform: &'a DAffine2,
	pub alpha_blending: &'a AlphaBlending,
}
#[derive(Debug)]
pub struct InstanceMut<'a, T> {
	pub id: &'a mut InstanceId,
	pub instance: &'a mut T,
	pub transform: &'a mut DAffine2,
	pub alpha_blending: &'a mut AlphaBlending,
}

// GRAPHIC ELEMENT
impl Transform for GraphicElement {
	fn transform(&self) -> DAffine2 {
		match self {
			GraphicElement::GraphicGroup(group) => group.transform(),
			GraphicElement::VectorData(vector_data) => vector_data.transform(),
			GraphicElement::RasterFrame(frame) => frame.transform(),
		}
	}
}
impl TransformMut for GraphicElement {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			GraphicElement::GraphicGroup(group) => group.transform_mut(),
			GraphicElement::VectorData(vector_data) => vector_data.transform_mut(),
			GraphicElement::RasterFrame(frame) => frame.transform_mut(),
		}
	}
}

// GRAPHIC GROUP
impl Transform for Instance<'_, GraphicGroup> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
}
impl Transform for InstanceMut<'_, GraphicGroup> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
}
impl TransformMut for InstanceMut<'_, GraphicGroup> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform
	}
}

// GRAPHIC GROUP TABLE
impl Transform for GraphicGroupTable {
	fn transform(&self) -> DAffine2 {
		self.one_instance().transform()
	}
}
impl TransformMut for GraphicGroupTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED"))
	}
}

// TEXTURE FRAME
impl Transform for Instance<'_, TextureFrame> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
}
impl Transform for InstanceMut<'_, TextureFrame> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
}
impl TransformMut for InstanceMut<'_, TextureFrame> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform
	}
}

// TEXTURE FRAME TABLE
impl Transform for TextureFrameTable {
	fn transform(&self) -> DAffine2 {
		self.one_instance().transform()
	}
}
impl TransformMut for TextureFrameTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED"))
	}
}

// IMAGE FRAME
impl<P: Pixel> Transform for Instance<'_, ImageFrame<P>> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.transform.transform_point2(pivot)
	}
}
impl<P: Pixel> Transform for InstanceMut<'_, ImageFrame<P>> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.transform.transform_point2(pivot)
	}
}
impl<P: Pixel> TransformMut for InstanceMut<'_, ImageFrame<P>> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform
	}
}

// IMAGE FRAME TABLE
impl<P: Pixel> Transform for ImageFrameTable<P>
where
	P: dyn_any::StaticType,
	P::Static: Pixel,
	GraphicElement: From<ImageFrame<P>>,
{
	fn transform(&self) -> DAffine2 {
		self.one_instance().transform()
	}
}
impl<P: Pixel> TransformMut for ImageFrameTable<P>
where
	P: dyn_any::StaticType,
	P::Static: Pixel,
	GraphicElement: From<ImageFrame<P>>,
{
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED"))
	}
}

// VECTOR DATA
impl Transform for Instance<'_, VectorData> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.transform.transform_point2(self.instance.layerspace_pivot(pivot))
	}
}
impl Transform for InstanceMut<'_, VectorData> {
	fn transform(&self) -> DAffine2 {
		*self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.transform.transform_point2(self.instance.layerspace_pivot(pivot))
	}
}
impl TransformMut for InstanceMut<'_, VectorData> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform
	}
}

// VECTOR DATA TABLE
impl Transform for VectorDataTable {
	fn transform(&self) -> DAffine2 {
		self.one_instance().transform()
	}
}
impl TransformMut for VectorDataTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED"))
	}
}

// RASTER FRAME
impl Transform for RasterFrame {
	fn transform(&self) -> DAffine2 {
		match self {
			RasterFrame::ImageFrame(image_frame) => image_frame.transform(),
			RasterFrame::TextureFrame(texture_frame) => texture_frame.transform(),
		}
	}
}
impl TransformMut for RasterFrame {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			RasterFrame::ImageFrame(image_frame) => image_frame.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED")),
			RasterFrame::TextureFrame(texture_frame) => texture_frame.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED")),
		}
	}
}
