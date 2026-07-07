use core_types::list::{Item, List};
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

/// Unwraps a ranked wire's item into its bare element for a legacy connector that predates ranked wires, discarding attributes.
#[node_macro::node(category(""), skip_impl)]
fn unwrap_item<'i, T: 'i + Send>(_: impl Ctx, value: Item<T>) -> T {
	value.into_element()
}

/// The adapter slot inserted ahead of each ranked connector: wraps a bare value onto the wire as an `Item`, or passes an
/// already ranked `Item` or `List` wire through unchanged. Sanctioned element conversions register under the same identifier.
#[node_macro::node(category(""), skip_impl)]
fn field_adapter<'i, T: 'i + Send + Into<O>, O: 'i + Send>(_: impl Ctx, value: T, _out_ty: PhantomData<O>) -> O {
	value.into()
}

/// Converts an `Item` wire's element to a different element type it can produce, letting a convertible wire feed an
/// `Item` connector whose element type it does not match by identity.
#[node_macro::node(category(""), skip_impl)]
fn field_adapter_convert<'i, T: 'i + Send + Into<E>, E: 'i + Send>(_: impl Ctx, value: Item<T>, _element_ty: PhantomData<E>) -> Item<E> {
	let (value, attributes) = value.into_parts();
	Item::from_parts(value.into(), attributes)
}

/// The `List` counterpart of `field_adapter_convert`, converting every element to a different element type it can produce.
#[node_macro::node(category(""), skip_impl)]
fn field_adapter_convert_list<'i, T: 'i + Send + Into<E>, E: 'i + Send>(_: impl Ctx, value: List<T>, _element_ty: PhantomData<E>) -> List<E> {
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

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	pub fn passthrough_node() {
		assert_eq!(passthrough((), &4), &4);
	}
}
