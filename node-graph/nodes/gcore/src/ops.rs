use core_types::ExtractAll;
use core_types::runtime::SourceFuture;
use core_types::{Ctx, ops::Convert, ops::ConvertAsync, transform::Footprint};
use std::marker::PhantomData;

/// Passes-through the input value without changing it. This is useful for rerouting wires for organization purposes.
#[node_macro::node(category("General"), skip_impl)]
fn passthrough<'i, T: 'i + Send>(_: impl Ctx, content: T) -> T {
	content
}

#[node_macro::node(category(""), skip_impl)]
fn into<T: Send + Into<O>, O: Send>(_: impl Ctx, value: T, #[data] _out_ty: PhantomData<O>) -> O {
	value.into()
}

#[node_macro::node(category(""), skip_impl)]
fn convert<T: Send + Convert<O, C>, O: Send, C: Send>(ctx: impl Ctx + ExtractAll, value: T, converter: C, #[data] _out_ty: PhantomData<O>) -> O {
	value.convert(*ctx.try_footprint().unwrap_or(&Footprint::DEFAULT), converter)
}

#[node_macro::node(category(""), skip_impl)]
fn convert_async<T: Send + ConvertAsync<O, C>, O: Send + 'static, C: Send>(ctx: impl Ctx + ExtractAll, value: T, converter: C, #[data] _out_ty: PhantomData<O>) -> SourceFuture<O> {
	value.convert(*ctx.try_footprint().unwrap_or(&Footprint::DEFAULT), converter)
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	pub fn passthrough_node() {
		assert_eq!(passthrough(&(), &4), &4);
	}
}
