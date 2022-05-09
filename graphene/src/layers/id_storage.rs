use crate::DocumentError;
use serde::{Deserialize, Serialize};

type ElementId = u64;

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are
/// stored in the [layers](PathStorage::layers) field.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]

// TODO: Default is a bit weird because Layer does not implement Default. but we should not care because the empty vec is the default
pub struct UniqueElements<T> {
	/// The ID that will be assigned to the next layer that is added to the folder
	next_assignment_id: ElementId,
	/// The IDs of the [Layer]s contained within the Folder
	pub ids: Vec<ElementId>,
	/// The data contained in the folder
	elements: Vec<T>,
}

impl<T> UniqueElements<T> {
	/// When a insertion ID is provided, try to insert the element with the given ID.
	/// If that ID is already used, return `None`.
	/// When no insertion ID is provided, search for the next free ID and insert it with that.
	/// Negative values for `insert_index` represent distance from the end
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::shape_layer::ShapeLayer;
	/// # use graphite_graphene::layers::UniqueElements;
	/// # use graphite_graphene::layers::style::PathStyle;
	/// # use graphite_graphene::layers::layer_info::LayerDataType;
	/// let mut folder = UniqueElements::default();
	///
	/// // Create two layers to be added to the folder
	/// let mut shape_layer = ShapeLayer::rectangle(PathStyle::default());
	/// let mut folder_layer = UniqueElements::default();
	///
	/// folder.add(shape_layer.into(), None, -1);
	/// folder.add(folder_layer.into(), Some(123), 0);
	/// ```
	pub fn add(&mut self, layer: T, id: Option<ElementId>, insert_index: isize) -> Option<ElementId> {
		let mut insert_index = insert_index as i128;

		if insert_index < 0 {
			insert_index = self.elements.len() as i128 + insert_index as i128 + 1;
		}

		if insert_index <= self.elements.len() as i128 && insert_index >= 0 {
			if let Some(id) = id {
				self.next_assignment_id = id;
			}
			if self.ids.contains(&self.next_assignment_id) {
				return None;
			}

			let id = self.next_assignment_id;
			self.elements.insert(insert_index as usize, layer);
			self.ids.insert(insert_index as usize, id);

			// Linear probing for collision avoidance
			while self.ids.contains(&self.next_assignment_id) {
				self.next_assignment_id += 1;
			}

			Some(id)
		} else {
			None
		}
	}

	/// Remove an element with a given element ID from the within this container.
	/// This operation will fail if `id` is not present within this container.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::UniqueElements;
	/// let mut folder = PathStorage::default();
	///
	/// // Try to remove a layer that does not exist
	/// assert!(folder.remove_layer(123).is_err());
	///
	/// // Add another folder to the folder
	/// folder.add_layer(PathStorage::default().into(), Some(123), -1);
	///
	/// // Try to remove that folder again
	/// assert!(folder.remove_layer(123).is_ok());
	/// assert_eq!(folder.layers().len(), 0)
	/// ```
	pub fn remove(&mut self, id: ElementId) -> Result<(), DocumentError> {
		let pos = self.position_of_element(id)?;
		self.elements.remove(pos);
		self.ids.remove(pos);
		Ok(())
	}

	/// Returns a list of [ElementId]s in the within this container.
	pub fn ids(&self) -> &[ElementId] {
		self.ids.as_slice()
	}

	/// Get references to all the [T]s in the within this container.
	pub fn elements(&self) -> &[T] {
		self.elements.as_slice()
	}

	/// Get mutable references to all the [T]s in the within this container.
	pub fn elements_mut(&mut self) -> &mut [T] {
		self.elements.as_mut_slice()
	}

	/// Get a single element with a given element ID from the within this container.
	pub fn element_by_id(&self, id: ElementId) -> Option<&T> {
		let pos = self.position_of_element(id).ok()?;
		Some(&self.elements[pos])
	}

	/// Get a mutable reference to a single element with a given element ID from the within this container.
	pub fn element_by_id_mut(&mut self, id: ElementId) -> Option<&mut T> {
		let pos = self.position_of_element(id).ok()?;
		Some(&mut self.elements[pos])
	}

	/// Get an element based on its index
	pub fn element_by_index(&self, index: usize) -> Option<&T> {
		self.elements.get(index)
	}

	/// Get a mutable element based on its index
	pub fn element_by_index_mut(&mut self, index: usize) -> Option<&mut T> {
		self.elements.get_mut(index)
	}

	pub fn last_element(&self) -> Option<&T> {
		self.elements.last()
	}

	pub fn last_element_mut(&mut self) -> Option<&mut T> {
		self.elements.last_mut()
	}

	pub fn len(&self) -> usize {
		self.elements.len()
	}

	pub fn is_empty(&self) -> bool {
		self.elements.is_empty()
	}

	/// Returns `true` if this contains an element with the given [ElementId].
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::UniqueElements;
	/// let mut folder = UniqueElements::default();
	///
	/// // Search for an id that does not exist
	/// assert!(!folder.contains(123));
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(UniqueElements::default().into(), Some(123), -1);
	///
	/// // Search for the id "123"
	/// assert!(folder.contains(123));
	/// ```
	pub fn contains(&self, id: ElementId) -> bool {
		self.ids.contains(&id)
	}

	/// Tries to find the index of a layer with the given [ElementId] within the folder.
	/// This operation will fail if no layer with a matching ID is present in the folder.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::UniqueElements;
	/// let mut folder = UniqueElements::default();
	///
	/// // Search for an id that does not exist
	/// assert!(folder.position_of_element(123).is_err());
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(UniqueElements::default().into(), Some(123), -1);
	/// folder.add_layer(UniqueElements::default().into(), Some(42), -1);
	///
	/// assert_eq!(folder.position_of_element(123), Ok(0));
	/// assert_eq!(folder.position_of_element(42), Ok(1));
	/// ```
	pub fn position_of_element(&self, element_id: ElementId) -> Result<usize, DocumentError> {
		// TODO This is a linear search, could we speed this up?
		self.ids.iter().position(|x| *x == element_id).ok_or_else(|| DocumentError::LayerNotFound([element_id].into()))
	}
}
