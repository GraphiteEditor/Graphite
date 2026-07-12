use core_types::graphene_hash::CacheHash;
use core_types::list::{AttributeValueDyn, Bundle, Item, List};
use core_types::{Ctx, ExtractFootprint, ops::Convert, transform::Footprint};
use std::marker::PhantomData;

// Re-export TypeNode from core-types for convenience
pub use core_types::ops::TypeNode;

/// Passes-through the input value without changing it. This is useful for rerouting wires for organization purposes.
#[node_macro::node(category("General"), skip_impl)]
fn passthrough<'i, T: 'i + Send>(_: impl Ctx, content: T) -> T {
	content
}

#[node_macro::node(category(""), skip_impl)]
fn into<'i, T: 'i + Send + Into<O>, O: 'i + Send>(_: impl Ctx, value: T, _out_ty: PhantomData<O>) -> O {
	value.into()
}

/// Boxes a ranked wire's element into a type-erased attribute value, carrying the cell's attributes through the wire.
#[node_macro::node(category(""), skip_impl)]
fn item_to_attribute_value<'i, T: 'i + Clone + Send + Sync + Default + std::fmt::Debug + PartialEq + CacheHash + 'static>(
	_: impl Ctx,
	value: Item<T>,
	_element_ty: PhantomData<AttributeValueDyn>,
) -> Item<AttributeValueDyn> {
	let (element, attributes) = value.into_parts();
	Item::from_parts(AttributeValueDyn(Box::new(element)), attributes)
}

/// Boxes a whole `List` wire as one type-erased attribute value, for attributes whose per-item value is itself a collection.
#[node_macro::node(category(""), skip_impl)]
fn list_to_attribute_value<'i, T: 'i + Clone + Send + Sync + Default + std::fmt::Debug + PartialEq + CacheHash + 'static>(
	_: impl Ctx,
	value: T,
	_element_ty: PhantomData<AttributeValueDyn>,
) -> Item<AttributeValueDyn> {
	Item::new_from_element(AttributeValueDyn(Box::new(value)))
}

/// Wraps a whole `List` onto the wire as one rank-0 `Item<Bundle<T>>` so an entire collection can feed a connector that carries it as one opaque cell.
#[node_macro::node(category(""), skip_impl)]
fn bundle<'i, T: 'i + Send>(_: impl Ctx, value: List<T>) -> Item<Bundle<T>> {
	Item::new_from_element(Bundle(value))
}

/// Unwraps a `Bundle` wire back into the whole `List` it carries, restoring the collection after it passed through a connector as one opaque cell.
#[node_macro::node(category(""), skip_impl)]
fn unbundle<'i, T: 'i + Send>(_: impl Ctx, value: Item<Bundle<T>>) -> List<T> {
	value.into_element().0
}

/// The adapter slot inserted ahead of each ranked connector: wraps a bare value onto the wire as an `Item`, or passes an
/// already ranked `Item` or `List` wire through unchanged. Sanctioned element conversions register under the same identifier.
#[node_macro::node(category(""), skip_impl)]
fn input_adapter<'i, T: 'i + Send + Into<O>, O: 'i + Send>(_: impl Ctx, value: T, _out_ty: PhantomData<O>) -> O {
	value.into()
}

/// Converts an `Item` wire's element to a different element type it can produce, letting a convertible wire feed an
/// `Item` connector whose element type it does not match by identity.
#[node_macro::node(category(""), skip_impl)]
fn input_adapter_convert<'i, T: 'i + Send + Into<E>, E: 'i + Send>(_: impl Ctx, value: Item<T>, _element_ty: PhantomData<E>) -> Item<E> {
	let (value, attributes) = value.into_parts();
	Item::from_parts(value.into(), attributes)
}

/// The `List` counterpart of `input_adapter_convert`, converting every element to a different element type it can produce.
#[node_macro::node(category(""), skip_impl)]
fn input_adapter_convert_list<'i, T: 'i + Send + Into<E>, E: 'i + Send>(_: impl Ctx, value: List<T>, _element_ty: PhantomData<E>) -> List<E> {
	value
		.into_iter()
		.map(|item| {
			let (value, attributes) = item.into_parts();
			Item::from_parts(value.into(), attributes)
		})
		.collect()
}

#[node_macro::node(category(""), skip_impl)]
async fn convert<'i, T: 'i + Send + Convert<O, C>, O: 'i + Send, C: 'i + Send>(ctx: impl Ctx + ExtractFootprint, value: T, converter: C, _out_ty: PhantomData<O>) -> O {
	value.convert(*ctx.try_footprint().unwrap_or(&Footprint::DEFAULT), converter).await
}

/// The `Convert`-based counterpart of `input_adapter_convert`, casting an `Item` wire's element to a connector's numeric element type.
#[node_macro::node(category(""), skip_impl)]
async fn input_adapter_cast<'i, T: 'i + Send + Convert<E, ()>, E: 'i + Send>(ctx: impl Ctx + ExtractFootprint, value: Item<T>, _element_ty: PhantomData<E>) -> Item<E> {
	let footprint = *ctx.try_footprint().unwrap_or(&Footprint::DEFAULT);
	let (value, attributes) = value.into_parts();

	Item::from_parts(value.convert(footprint, ()).await, attributes)
}

/// The bare-wire counterpart of `input_adapter_cast`, wrapping a value onto the ranked wire as an `Item` of the connector's element type.
#[node_macro::node(category(""), skip_impl)]
async fn input_adapter_cast_wrap<'i, T: 'i + Send + Convert<E, ()>, E: 'i + Send>(ctx: impl Ctx + ExtractFootprint, value: T, _element_ty: PhantomData<E>) -> Item<E> {
	let footprint = *ctx.try_footprint().unwrap_or(&Footprint::DEFAULT);

	Item::new_from_element(value.convert(footprint, ()).await)
}

/// The `List` counterpart of `input_adapter_cast`, casting every element to the connector's numeric element type.
#[node_macro::node(category(""), skip_impl)]
async fn input_adapter_cast_list<'i, T: 'i + Send + Convert<E, ()>, E: 'i + Send>(ctx: impl Ctx + ExtractFootprint, value: List<T>, _element_ty: PhantomData<E>) -> List<E> {
	let footprint = *ctx.try_footprint().unwrap_or(&Footprint::DEFAULT);

	let mut result = List::default();
	for item in value.into_iter() {
		let (value, attributes) = item.into_parts();
		result.push(Item::from_parts(value.convert(footprint, ()).await, attributes));
	}

	result
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	pub fn passthrough_node() {
		assert_eq!(passthrough((), &4), &4);
	}
}
