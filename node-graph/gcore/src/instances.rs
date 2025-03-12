use crate::application_io::TextureFrameTable;
use crate::raster::Pixel;
use crate::raster::image::{Image, ImageFrameTable};
use crate::transform::{Transform, TransformMut};
use crate::uuid::NodeId;
use crate::vector::{InstanceId, VectorDataTable};
use crate::{AlphaBlending, GraphicElement, RasterFrame};

use dyn_any::StaticType;

use glam::DAffine2;
use std::hash::Hash;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Instances<T> {
	id: Vec<InstanceId>,
	#[serde(alias = "instances")]
	instance: Vec<T>,
	#[serde(default = "one_daffine2_default")]
	transform: Vec<DAffine2>,
	#[serde(default = "one_alpha_blending_default")]
	alpha_blending: Vec<AlphaBlending>,
	#[serde(default = "one_source_node_id_default")]
	source_node_id: Vec<Option<NodeId>>,
}

impl<T> Instances<T> {
	pub fn new(instance: T) -> Self {
		Self {
			id: vec![InstanceId::generate()],
			instance: vec![instance],
			transform: vec![DAffine2::IDENTITY],
			alpha_blending: vec![AlphaBlending::default()],
			source_node_id: vec![None],
		}
	}

	pub fn empty() -> Self {
		Self {
			id: Vec::new(),
			instance: Vec::new(),
			transform: Vec::new(),
			alpha_blending: Vec::new(),
			source_node_id: Vec::new(),
		}
	}

	pub fn push(&mut self, instance: T) -> InstanceMut<T> {
		self.id.push(InstanceId::generate());
		self.instance.push(instance);
		self.transform.push(DAffine2::IDENTITY);
		self.alpha_blending.push(AlphaBlending::default());
		self.source_node_id.push(None);

		InstanceMut {
			id: self.id.last_mut().expect("Shouldn't be empty"),
			instance: self.instance.last_mut().expect("Shouldn't be empty"),
			transform: self.transform.last_mut().expect("Shouldn't be empty"),
			alpha_blending: self.alpha_blending.last_mut().expect("Shouldn't be empty"),
			source_node_id: self.source_node_id.last_mut().expect("Shouldn't be empty"),
		}
	}

	pub fn push_instance(&mut self, instance: Instance<T>) -> InstanceMut<T>
	where
		T: Clone,
	{
		self.id.push(*instance.id);
		self.instance.push(instance.instance.clone());
		self.transform.push(*instance.transform);
		self.alpha_blending.push(*instance.alpha_blending);
		self.source_node_id.push(*instance.source_node_id);

		InstanceMut {
			id: self.id.last_mut().expect("Shouldn't be empty"),
			instance: self.instance.last_mut().expect("Shouldn't be empty"),
			transform: self.transform.last_mut().expect("Shouldn't be empty"),
			alpha_blending: self.alpha_blending.last_mut().expect("Shouldn't be empty"),
			source_node_id: self.source_node_id.last_mut().expect("Shouldn't be empty"),
		}
	}

	pub fn one_instance(&self) -> Instance<T> {
		Instance {
			id: self.id.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			instance: self.instance.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			transform: self.transform.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			alpha_blending: self.alpha_blending.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			source_node_id: self.source_node_id.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
		}
	}

	pub fn one_instance_mut(&mut self) -> InstanceMut<T> {
		let length = self.instance.len();

		InstanceMut {
			id: self.id.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
			instance: self.instance.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
			transform: self.transform.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
			alpha_blending: self.alpha_blending.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
			source_node_id: self.source_node_id.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", length)),
		}
	}

	pub fn instances(&self) -> impl DoubleEndedIterator<Item = Instance<T>> {
		self.id
			.iter()
			.zip(self.instance.iter())
			.zip(self.transform.iter())
			.zip(self.alpha_blending.iter())
			.zip(self.source_node_id.iter())
			.map(|((((id, instance), transform), alpha_blending), source_node_id)| Instance {
				id,
				instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	pub fn instances_mut(&mut self) -> impl DoubleEndedIterator<Item = InstanceMut<T>> {
		self.id
			.iter_mut()
			.zip(self.instance.iter_mut())
			.zip(self.transform.iter_mut())
			.zip(self.alpha_blending.iter_mut())
			.zip(self.source_node_id.iter_mut())
			.map(|((((id, instance), transform), alpha_blending), source_node_id)| InstanceMut {
				id,
				instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}
}

impl<T: Default + Hash + 'static> Default for Instances<T> {
	fn default() -> Self {
		// TODO: Remove once all types have been converted to tables
		let converted_to_tables = [TypeId::of::<crate::Artboard>(), TypeId::of::<crate::GraphicElement>()];

		use core::any::TypeId;
		if converted_to_tables.contains(&TypeId::of::<T>()) {
			// TODO: Remove the 'static trait bound when this special casing is removed by making all types return empty
			Self::empty()
		} else {
			Self::new(T::default())
		}
	}
}

impl<T: Hash> core::hash::Hash for Instances<T> {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
		for instance in &self.instance {
			instance.hash(state);
		}
	}
}

impl<T: PartialEq> PartialEq for Instances<T> {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id && self.instance.len() == other.instance.len() && { self.instance.iter().zip(other.instance.iter()).all(|(a, b)| a == b) }
	}
}

#[cfg(feature = "dyn-any")]
unsafe impl<T: StaticType + 'static> StaticType for Instances<T> {
	type Static = Instances<T>;
}

fn one_daffine2_default() -> Vec<DAffine2> {
	vec![DAffine2::IDENTITY]
}
fn one_alpha_blending_default() -> Vec<AlphaBlending> {
	vec![AlphaBlending::default()]
}
fn one_source_node_id_default() -> Vec<Option<NodeId>> {
	vec![None]
}

#[derive(Copy, Clone, Debug)]
pub struct Instance<'a, T> {
	pub id: &'a InstanceId,
	pub instance: &'a T,
	pub transform: &'a DAffine2,
	pub alpha_blending: &'a AlphaBlending,
	pub source_node_id: &'a Option<NodeId>,
}
#[derive(Debug)]
pub struct InstanceMut<'a, T> {
	pub id: &'a mut InstanceId,
	pub instance: &'a mut T,
	pub transform: &'a mut DAffine2,
	pub alpha_blending: &'a mut AlphaBlending,
	pub source_node_id: &'a mut Option<NodeId>,
}

// VECTOR DATA TABLE
impl Transform for VectorDataTable {
	fn transform(&self) -> DAffine2 {
		*self.one_instance().transform
	}
}
impl TransformMut for VectorDataTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.one_instance_mut().transform
	}
}

// TEXTURE FRAME TABLE
impl Transform for TextureFrameTable {
	fn transform(&self) -> DAffine2 {
		*self.one_instance().transform
	}
}
impl TransformMut for TextureFrameTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.one_instance_mut().transform
	}
}

// IMAGE FRAME TABLE
impl<P: Pixel> Transform for ImageFrameTable<P>
where
	GraphicElement: From<Image<P>>,
{
	fn transform(&self) -> DAffine2 {
		*self.one_instance().transform
	}
}
impl<P: Pixel> TransformMut for ImageFrameTable<P>
where
	GraphicElement: From<Image<P>>,
{
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self.one_instance_mut().transform
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
			RasterFrame::ImageFrame(image_frame) => image_frame.transform_mut(),
			RasterFrame::TextureFrame(texture_frame) => texture_frame.transform_mut(),
		}
	}
}
