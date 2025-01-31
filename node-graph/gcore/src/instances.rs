use crate::vector::InstanceId;
use crate::{AlphaBlending, GraphicElement};

use dyn_any::StaticType;
use glam::DAffine2;
use std::hash::Hash;

#[derive(Copy, Clone, Debug)]
pub struct Instance<'a, T> {
	pub id: &'a InstanceId,
	pub instance: &'a T,
	pub transform: &'a DAffine2,
	pub alpha_blending: &'a AlphaBlending,
}
pub struct InstanceMut<'a, T> {
	pub id: &'a mut InstanceId,
	pub instance: &'a mut T,
	pub transform: &'a mut DAffine2,
	pub alpha_blending: &'a mut AlphaBlending,
}

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
		#[cfg(debug_assertions)]
		let length = self.instance.len();
		#[cfg(not(debug_assertions))]
		let length = '?';

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

	// pub fn id(&self) -> impl Iterator<Item = InstanceId> + '_ {
	// 	self.id.iter().copied()
	// }

	// pub fn push(&mut self, id: InstanceId, instance: T) {
	// 	self.id.push(id);
	// 	self.instances.push(instance);
	// }

	// pub fn replace_all(&mut self, id: InstanceId, instance: T) {
	// 	let mut instance = instance;

	// 	for (old_id, old_instance) in self.id.iter_mut().zip(self.instances.iter_mut()) {
	// 		let mut new_id = id;
	// 		std::mem::swap(old_id, &mut new_id);
	// 		std::mem::swap(&mut instance, old_instance);
	// 	}
	// }
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
