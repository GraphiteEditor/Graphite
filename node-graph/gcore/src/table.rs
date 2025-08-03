use crate::AlphaBlending;
use crate::transform::ApplyTransform;
use crate::uuid::NodeId;
use dyn_any::StaticType;
use glam::DAffine2;
use std::hash::Hash;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Table<T> {
	#[serde(alias = "instances", alias = "instance")]
	element: Vec<T>,
	#[serde(default = "one_daffine2_default")]
	transform: Vec<DAffine2>,
	#[serde(default = "one_alpha_blending_default")]
	alpha_blending: Vec<AlphaBlending>,
	#[serde(default = "one_source_node_id_default")]
	source_node_id: Vec<Option<NodeId>>,
}

impl<T> Table<T> {
	pub fn new(instance: T) -> Self {
		Self {
			element: vec![instance],
			transform: vec![DAffine2::IDENTITY],
			alpha_blending: vec![AlphaBlending::default()],
			source_node_id: vec![None],
		}
	}

	pub fn new_instance(instance: Instance<T>) -> Self {
		Self {
			element: vec![instance.element],
			transform: vec![instance.transform],
			alpha_blending: vec![instance.alpha_blending],
			source_node_id: vec![instance.source_node_id],
		}
	}

	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			element: Vec::with_capacity(capacity),
			transform: Vec::with_capacity(capacity),
			alpha_blending: Vec::with_capacity(capacity),
			source_node_id: Vec::with_capacity(capacity),
		}
	}

	pub fn push(&mut self, instance: Instance<T>) {
		self.element.push(instance.element);
		self.transform.push(instance.transform);
		self.alpha_blending.push(instance.alpha_blending);
		self.source_node_id.push(instance.source_node_id);
	}

	pub fn extend(&mut self, table: Table<T>) {
		self.element.extend(table.element);
		self.transform.extend(table.transform);
		self.alpha_blending.extend(table.alpha_blending);
		self.source_node_id.extend(table.source_node_id);
	}

	pub fn instance_iter(self) -> impl DoubleEndedIterator<Item = Instance<T>> {
		self.element
			.into_iter()
			.zip(self.transform)
			.zip(self.alpha_blending)
			.zip(self.source_node_id)
			.map(|(((instance, transform), alpha_blending), source_node_id)| Instance {
				element: instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	pub fn instance_ref_iter(&self) -> impl DoubleEndedIterator<Item = InstanceRef<'_, T>> + Clone {
		self.element
			.iter()
			.zip(self.transform.iter())
			.zip(self.alpha_blending.iter())
			.zip(self.source_node_id.iter())
			.map(|(((instance, transform), alpha_blending), source_node_id)| InstanceRef {
				element: instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	pub fn instance_mut_iter(&mut self) -> impl DoubleEndedIterator<Item = InstanceMut<'_, T>> {
		self.element
			.iter_mut()
			.zip(self.transform.iter_mut())
			.zip(self.alpha_blending.iter_mut())
			.zip(self.source_node_id.iter_mut())
			.map(|(((instance, transform), alpha_blending), source_node_id)| InstanceMut {
				element: instance,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	pub fn get(&self, index: usize) -> Option<InstanceRef<'_, T>> {
		if index >= self.element.len() {
			return None;
		}

		Some(InstanceRef {
			element: &self.element[index],
			transform: &self.transform[index],
			alpha_blending: &self.alpha_blending[index],
			source_node_id: &self.source_node_id[index],
		})
	}

	pub fn get_mut(&mut self, index: usize) -> Option<InstanceMut<'_, T>> {
		if index >= self.element.len() {
			return None;
		}

		Some(InstanceMut {
			element: &mut self.element[index],
			transform: &mut self.transform[index],
			alpha_blending: &mut self.alpha_blending[index],
			source_node_id: &mut self.source_node_id[index],
		})
	}

	pub fn len(&self) -> usize {
		self.element.len()
	}

	pub fn is_empty(&self) -> bool {
		self.element.is_empty()
	}
}

impl<T> Default for Table<T> {
	fn default() -> Self {
		Self {
			element: Vec::new(),
			transform: Vec::new(),
			alpha_blending: Vec::new(),
			source_node_id: Vec::new(),
		}
	}
}

impl<T: Hash> Hash for Table<T> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		for instance in &self.element {
			instance.hash(state);
		}
	}
}

impl<T> ApplyTransform for Table<T> {
	fn apply_transform(&mut self, modification: &DAffine2) {
		for transform in &mut self.transform {
			*transform *= *modification;
		}
	}

	fn left_apply_transform(&mut self, modification: &DAffine2) {
		for transform in &mut self.transform {
			*transform = *modification * *transform;
		}
	}
}

impl<T: PartialEq> PartialEq for Table<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element.len() == other.element.len() && { self.element.iter().zip(other.element.iter()).all(|(a, b)| a == b) }
	}
}

unsafe impl<T: StaticType + 'static> StaticType for Table<T> {
	type Static = Table<T>;
}

impl<T> FromIterator<Instance<T>> for Table<T> {
	fn from_iter<I: IntoIterator<Item = Instance<T>>>(iter: I) -> Self {
		let iter = iter.into_iter();
		let (lower, _) = iter.size_hint();
		let mut table = Self::with_capacity(lower);
		for instance in iter {
			table.push(instance);
		}
		table
	}
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
	pub element: &'a T,
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
			element: self.element.clone(),
			transform: *self.transform,
			alpha_blending: *self.alpha_blending,
			source_node_id: *self.source_node_id,
		}
	}
}

#[derive(Debug)]
pub struct InstanceMut<'a, T> {
	pub element: &'a mut T,
	pub transform: &'a mut DAffine2,
	pub alpha_blending: &'a mut AlphaBlending,
	pub source_node_id: &'a mut Option<NodeId>,
}

#[derive(Copy, Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Instance<T> {
	#[serde(alias = "instance")]
	pub element: T,
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
			element: self.element.into(),
			transform: self.transform,
			alpha_blending: self.alpha_blending,
			source_node_id: self.source_node_id,
		}
	}

	pub fn to_instance_ref(&self) -> InstanceRef<'_, T> {
		InstanceRef {
			element: &self.element,
			transform: &self.transform,
			alpha_blending: &self.alpha_blending,
			source_node_id: &self.source_node_id,
		}
	}

	pub fn to_instance_mut(&mut self) -> InstanceMut<'_, T> {
		InstanceMut {
			element: &mut self.element,
			transform: &mut self.transform,
			alpha_blending: &mut self.alpha_blending,
			source_node_id: &mut self.source_node_id,
		}
	}

	pub fn to_table(self) -> Table<T> {
		Table {
			element: vec![self.element],
			transform: vec![self.transform],
			alpha_blending: vec![self.alpha_blending],
			source_node_id: vec![self.source_node_id],
		}
	}
}
