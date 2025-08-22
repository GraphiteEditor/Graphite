use crate::context::{CloneVarArgs, Context, ContextFeatures, Ctx, ExtractAll};
use crate::gradient::GradientStops;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::Table;
use crate::vector::Vector;
use crate::{Graphic, OwnedContextImpl};
use core::f64;
use glam::{DAffine2, DVec2};
use graphene_core_shaders::color::Color;

/// Node for filtering context features based on requirements
/// This node is inserted by the compiler to "zero out" unused context parts
#[node_macro::node(category("Internal"))]
async fn context_modification<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> (),
		Context -> bool,
		Context -> u32,
		Context -> f32,
		Context -> f64,
		Context -> DAffine2,
		Context -> DVec2,
		Context -> Table<Vector>,
		Context -> Table<Graphic>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	value: impl Node<Context<'static>, Output = T>,
	features_to_keep: ContextFeatures,
) -> T {
	let new_context = OwnedContextImpl::from_flags(ctx, features_to_keep);

	value.eval(Some(new_context.into())).await
}
