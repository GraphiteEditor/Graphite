use crate::blending::AlphaBlending;
use crate::uuid::NodeId;
use dyn_any::StaticType;
use glam::DAffine2;
use std::hash::Hash;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Instances<T> {
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
			instance: vec![instance],
			transform: vec![DAffine2::IDENTITY],
			alpha_blending: vec![AlphaBlending::default()],
			source_node_id: vec![None],
		}
	}

	pub fn push(&mut self, instance: Instance<T>) {
		self.instance.push(instance.instance);
		self.transform.push(instance.transform);
		self.alpha_blending.push(instance.alpha_blending);
		self.source_node_id.push(instance.source_node_id);
	}

	pub fn extend(&mut self, instances: Instances<T>) {
		self.instance.extend(instances.instance);
		self.transform.extend(instances.transform);
		self.alpha_blending.extend(instances.alpha_blending);
		self.source_node_id.extend(instances.source_node_id);
	}

	pub fn instance_iter(self) -> impl DoubleEndedIterator<Item = Instance<T>> {
		self.instance
			.into_iter()
			.zip(self.transform)
			.zip(self.alpha_blending)
			.zip(self.source_node_id)
			.map(|(((instance, transform), alpha_blending), source_node_id)| Instance {
				instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	pub fn instance_ref_iter(&self) -> impl DoubleEndedIterator<Item = InstanceRef<'_, T>> + Clone {
		self.instance
			.iter()
			.zip(self.transform.iter())
			.zip(self.alpha_blending.iter())
			.zip(self.source_node_id.iter())
			.map(|(((instance, transform), alpha_blending), source_node_id)| InstanceRef {
				instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	pub fn instance_mut_iter(&mut self) -> impl DoubleEndedIterator<Item = InstanceMut<'_, T>> {
		self.instance
			.iter_mut()
			.zip(self.transform.iter_mut())
			.zip(self.alpha_blending.iter_mut())
			.zip(self.source_node_id.iter_mut())
			.map(|(((instance, transform), alpha_blending), source_node_id)| InstanceMut {
				instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	pub fn get(&self, index: usize) -> Option<InstanceRef<'_, T>> {
		if index >= self.instance.len() {
			return None;
		}

		Some(InstanceRef {
			instance: &self.instance[index],
			transform: &self.transform[index],
			alpha_blending: &self.alpha_blending[index],
			source_node_id: &self.source_node_id[index],
		})
	}

	pub fn get_mut(&mut self, index: usize) -> Option<InstanceMut<'_, T>> {
		if index >= self.instance.len() {
			return None;
		}

		Some(InstanceMut {
			instance: &mut self.instance[index],
			transform: &mut self.transform[index],
			alpha_blending: &mut self.alpha_blending[index],
			source_node_id: &mut self.source_node_id[index],
		})
	}

	pub fn len(&self) -> usize {
		self.instance.len()
	}

	pub fn is_empty(&self) -> bool {
		self.instance.is_empty()
	}
}

impl<T> Default for Instances<T> {
	fn default() -> Self {
		Self {
			instance: Vec::new(),
			transform: Vec::new(),
			alpha_blending: Vec::new(),
			source_node_id: Vec::new(),
		}
	}
}

impl<T: Hash> Hash for Instances<T> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		for instance in &self.instance {
			instance.hash(state);
		}
	}
}

impl<T: PartialEq> PartialEq for Instances<T> {
	fn eq(&self, other: &Self) -> bool {
		self.instance.len() == other.instance.len() && { self.instance.iter().zip(other.instance.iter()).all(|(a, b)| a == b) }
	}
}

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

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct InstanceRef<'a, T> {
	pub instance: &'a T,
	pub transform: &'a DAffine2,
	pub alpha_blending: &'a AlphaBlending,
	pub source_node_id: &'a Option<NodeId>,
}

impl<T> InstanceRef<'_, T> {
	pub fn to_instance_cloned(self) -> Instance<T>
	where
		T: Clone,
	{
		Instance {
			instance: self.instance.clone(),
			transform: *self.transform,
			alpha_blending: *self.alpha_blending,
			source_node_id: *self.source_node_id,
		}
	}
}

#[derive(Debug)]
pub struct InstanceMut<'a, T> {
	pub instance: &'a mut T,
	pub transform: &'a mut DAffine2,
	pub alpha_blending: &'a mut AlphaBlending,
	pub source_node_id: &'a mut Option<NodeId>,
}

#[derive(Copy, Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Instance<T> {
	pub instance: T,
	pub transform: DAffine2,
	pub alpha_blending: AlphaBlending,
	pub source_node_id: Option<NodeId>,
}

impl<T> Instance<T> {
	pub fn to_graphic_element<U>(self) -> Instance<U>
	where
		T: Into<U>,
	{
		Instance {
			instance: self.instance.into(),
			transform: self.transform,
			alpha_blending: self.alpha_blending,
			source_node_id: self.source_node_id,
		}
	}

	pub fn to_instance_ref(&self) -> InstanceRef<'_, T> {
		InstanceRef {
			instance: &self.instance,
			transform: &self.transform,
			alpha_blending: &self.alpha_blending,
			source_node_id: &self.source_node_id,
		}
	}

	pub fn to_instance_mut(&mut self) -> InstanceMut<'_, T> {
		InstanceMut {
			instance: &mut self.instance,
			transform: &mut self.transform,
			alpha_blending: &mut self.alpha_blending,
			source_node_id: &mut self.source_node_id,
		}
	}

	pub fn to_table(self) -> Instances<T> {
		Instances {
			instance: vec![self.instance],
			transform: vec![self.transform],
			alpha_blending: vec![self.alpha_blending],
			source_node_id: vec![self.source_node_id],
		}
	}
}
