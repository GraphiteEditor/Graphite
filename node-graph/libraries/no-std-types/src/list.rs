//! A zero-cost stand-in for `core_types::list::Item` used when node kernels are compiled for the GPU.
//!
//! Shader node kernels compile twice: under `std` against the real attribute-carrying `Item`, and under
//! `no_std` (SPIR-V) against this transparent wrapper. Only the element-access surface is provided, since
//! rust-gpu cannot allocate and attributes have no per-pixel meaning; attribute use fails the shader build.

/// A rank-0 wire value holding a single element, mirroring the element-access API of the real `Item`.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Item<T> {
	element: T,
}

impl<T> Item<T> {
	/// Constructs an item with the given element.
	pub fn new_from_element(element: T) -> Self {
		Self { element }
	}

	/// Returns a shared reference to this item's element.
	pub fn element(&self) -> &T {
		&self.element
	}

	/// Returns a mutable reference to this item's element.
	pub fn element_mut(&mut self) -> &mut T {
		&mut self.element
	}

	/// Consumes this item and returns the owned element.
	pub fn into_element(self) -> T {
		self.element
	}
}

impl<T> From<T> for Item<T> {
	fn from(element: T) -> Self {
		Self::new_from_element(element)
	}
}
