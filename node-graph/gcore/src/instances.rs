use crate::vector::InstanceId;
use crate::GraphicElement;

use dyn_any::StaticType;

use std::hash::Hash;

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

	pub fn one_item(&self) -> &T {
		self.instance.first().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {} (one_item)", self.instance.len()))
	}

	pub fn one_item_mut(&mut self) -> &mut T {
		let length = self.instance.len();
		self.instance.first_mut().unwrap_or_else(|| panic!("ONE INSTANCE EXPECTED, FOUND {} (one_item_mut)", length))
	}

	pub fn instances(&self) -> impl Iterator<Item = &T> {
		assert!(self.instance.len() == 1, "ONE INSTANCE EXPECTED, FOUND {} (instances)", self.instance.len());
		self.instance.iter()
	}

	pub fn instances_mut(&mut self) -> impl Iterator<Item = &mut T> {
		assert!(self.instance.len() == 1, "ONE INSTANCE EXPECTED, FOUND {} (instances_mut)", self.instance.len());
		self.instance.iter_mut()
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
