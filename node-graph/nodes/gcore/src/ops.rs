use core_types::list::Item;
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

/// Wraps a bare value onto a ranked wire as an `Item`, or passes an already ranked `Item` or `List` wire through unchanged.
#[node_macro::node(category(""), skip_impl)]
fn promote<'i, T: 'i + Send + Into<O>, O: 'i + Send>(_: impl Ctx, value: T, _out_ty: PhantomData<O>) -> O {
	value.into()
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
