use core_types::ExtractAll;
use core_types::runtime::SourceFuture;
use core_types::{Ctx, ops::Convert, ops::ConvertAsync, transform::Footprint};
use std::marker::PhantomData;

/// Passes-through the input value without changing it. This is useful for rerouting wires for organization purposes.
#[node_macro::node(category("General"), skip_impl)]
fn passthrough<T: Send>(_: impl Ctx, content: T) -> T {
	content
}

/// Shifts a whole wire value onto a connector's type through the std `Into` trait, serving the whole-`List` erasure onto `ListDyn` under the input adapter identifier.
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

/// The bare-wire counterpart of `input_adapter_cast`, wrapping a value onto the ranked wire as an `Item` of the connector's element type.
#[node_macro::node(category(""), skip_impl)]
async fn input_adapter_cast_wrap<'i, T: 'i + Send + Convert<E, ()>, E: 'i + Send>(ctx: impl Ctx + ExtractFootprint, value: T, _element_ty: PhantomData<E>) -> Item<E> {
	let footprint = *ctx.try_footprint().unwrap_or(&Footprint::DEFAULT);

	Item::new_from_element(value.convert(footprint, ()).await)
}

#[node_macro::node(category(""), skip_impl)]
async fn convert<'i, T: 'i + Send + Convert<O, C>, O: 'i + Send, C: 'i + Send>(ctx: impl Ctx + ExtractFootprint, value: T, converter: C, _out_ty: PhantomData<O>) -> O {
	value.convert(*ctx.try_footprint().unwrap_or(&Footprint::DEFAULT), converter).await
}
