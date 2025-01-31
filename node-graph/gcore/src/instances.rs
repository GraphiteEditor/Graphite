use crate::vector::InstanceId;
use crate::GraphicElement;

use dyn_any::StaticType;
use std::hash::Hash;

#[derive(Copy, Clone, Debug)]
pub struct Instance<'a, T> {
	pub id: &'a InstanceId,
	pub instance: &'a T,
}
pub struct InstanceMut<'a, T> {
	pub id: &'a mut InstanceId,
	pub instance: &'a mut T,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Instances<T>
where
	T: Into<GraphicElement> + StaticType + 'static,
{
	id: Vec<InstanceId>,
	#[serde(alias = "instances")]
	instance: Vec<T>,
}

impl<T: Into<GraphicElement> + StaticType + 'static> Instances<T> {
	pub fn new(instance: T) -> Self {
		Self {
			id: vec![InstanceId::generate()],
			instance: vec![instance],
		}
	}

	pub fn one_instance(&self) -> Instance<T> {
		Instance {
			id: self.id.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
			instance: self.instance.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {}", self.instance.len())),
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
		}
	}

	pub fn instances(&self) -> impl Iterator<Item = Instance<T>> {
		assert!(self.instance.len() == 1, "ONE INSTANCE EXPECTED, FOUND {} (instances)", self.instance.len());
		self.id.iter().zip(self.instance.iter()).map(|(id, instance)| Instance { id, instance })
	}

	pub fn instances_mut(&mut self) -> impl Iterator<Item = InstanceMut<T>> {
		assert!(self.instance.len() == 1, "ONE INSTANCE EXPECTED, FOUND {} (instances_mut)", self.instance.len());
		self.id.iter_mut().zip(self.instance.iter_mut()).map(|(id, instance)| InstanceMut { id, instance })
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
