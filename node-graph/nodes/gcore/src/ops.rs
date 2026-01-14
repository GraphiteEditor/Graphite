use core_types::{Ctx, ExtractFootprint, ops::Convert, transform::Footprint};
use std::marker::PhantomData;

// Re-export TypeNode from core-types for convenience
pub use core_types::ops::TypeNode;

// TODO: Rename to "Passthrough" and make this the node that users use, not the one defined in document_node_definitions.rs
/// Passes-through the input value without changing it.
/// This is useful for rerouting wires for organization purposes.
#[node_macro::node(skip_impl)]
fn identity<'i, T: 'i + Send>(value: T) -> T {
	value
}

#[node_macro::node(skip_impl)]
fn into<'i, T: 'i + Send + Into<O>, O: 'i + Send>(_: impl Ctx, value: T, _out_ty: PhantomData<O>) -> O {
	value.into()
}

#[node_macro::node(skip_impl)]
async fn convert<'i, T: 'i + Send + Convert<O, C>, O: 'i + Send, C: 'i + Send>(ctx: impl Ctx + ExtractFootprint, value: T, converter: C, _out_ty: PhantomData<O>) -> O {
	value.convert(*ctx.try_footprint().unwrap_or(&Footprint::DEFAULT), converter).await
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	pub fn identity_node() {
		assert_eq!(identity(&4), &4);
	}
}
