use core::f64;
use core_types::context::{Context, ContextFeatures, Ctx, DeriveCtx};
use core_types::gpoll::GPoll;
use core_types::list::{AttributeDyn, AttributeValueDyn, List, ListDyn};
use core_types::transform::Footprint;
use core_types::uuid::NodeId;
use core_types::Color;
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::GradientStops;
use graphic_types::{Artboard, Graphic, Vector};
use raster_types::{CPU, GPU, Raster};

/// Filters out what should be unused components of the context based on the specified requirements.
/// This node is inserted by the compiler to "zero out" unused context components.
#[node_macro::node(category(""))]
fn context_modification<T>(
	ctx: impl Ctx + DeriveCtx,
	/// The data to pass through, evaluated with the stripped down context.
	#[implementations(
		Context -> (),
		Context -> bool,
		Context -> u32,
		Context -> u64,
		Context -> f32,
		Context -> f64,
		Context -> String,
		Context -> DAffine2,
		Context -> Footprint,
		Context -> DVec2,
		Context -> List<String>,
		Context -> List<NodeId>,
		Context -> List<f64>,
		Context -> List<u8>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Artboard>,
		Context -> List<GradientStops>,
		Context -> AttributeDyn,
		Context -> AttributeValueDyn,
		Context -> ListDyn,
	)]
	value: impl Node<Context<'_>, Output = T>,
	/// The parts of the context to keep when evaluating the input value. All other parts are nullified.
	features_to_keep: ContextFeatures,
) -> GPoll<T> {
	let scope = ctx.scope().nullified(features_to_keep);
	value.eval(&ctx.nullified(features_to_keep, &scope))
}
