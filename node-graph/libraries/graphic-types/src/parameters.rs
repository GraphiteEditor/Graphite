//! Parameters riding an item as attributes, each read as its default when absent, which one listing per operation both writes and reads.

use core_types::graphene_hash::CacheHash;
use core_types::list::Item;
use std::any::Any;
use std::fmt::Debug;

/// What a value needs to ride an attribute.
pub(crate) trait ParameterValue: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static {}
impl<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static> ParameterValue for T {}

/// Handles each parameter of an operation, given the attribute key it rides and its default.
pub(crate) trait ParameterVisitor {
	fn parameter<T: ParameterValue>(&mut self, key: &str, value: &mut T, default: T);
}

/// Stamps each parameter that differs from its default onto an item, for rows whose attributes are part of their value.
pub(crate) struct WriteParameters<'a, T>(pub &'a mut Item<T>);

impl<T> ParameterVisitor for WriteParameters<'_, T> {
	fn parameter<U: ParameterValue>(&mut self, key: &str, value: &mut U, default: U) {
		if *value != default {
			self.0.set_attribute(key, value.clone());
		}
	}
}

/// Stamps every parameter onto an item, for an item on a wire, whose attributes mustn't come and go with their values.
pub(crate) struct StampParameters<'a, T>(pub &'a mut Item<T>);

impl<T> ParameterVisitor for StampParameters<'_, T> {
	fn parameter<U: ParameterValue>(&mut self, key: &str, value: &mut U, _default: U) {
		self.0.set_attribute(key, value.clone());
	}
}

/// Reads each parameter from the attribute that the lookup finds by key, or its default where there's none.
pub(crate) struct ReadParameters<'a>(pub &'a dyn Fn(&str) -> Option<&'a dyn Any>);

impl ParameterVisitor for ReadParameters<'_> {
	fn parameter<T: ParameterValue>(&mut self, key: &str, value: &mut T, default: T) {
		*value = (self.0)(key).and_then(|value| value.downcast_ref::<T>()).cloned().unwrap_or(default);
	}
}
