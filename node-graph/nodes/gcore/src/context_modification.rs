use core::f64;
use core_types::context::{CloneVarArgs, Context, ContextFeatures, Ctx, ExtractAll};
use core_types::table::Table;
use core_types::transform::Footprint;
use core_types::uuid::NodeId;
use core_types::{Color, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::GradientStops;
use graphic_types::{Artboard, Graphic, Vector};
use raster_types::{CPU, GPU, Raster};

/// Filters out what should be unused components of the context based on the specified requirements.
/// This node is inserted by the compiler to "zero out" unused context components.
#[node_macro::node(category(""))]
async fn context_modification<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
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
		Context -> Table<String>,
		Context -> Table<NodeId>,
		Context -> Table<f64>,
		Context -> Table<u8>,
		Context -> Table<Vector>,
		Context -> Table<Graphic>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> Table<Artboard>,
		Context -> Table<GradientStops>,
	)]
	value: impl Node<Context<'static>, Output = T>,
	/// The parts of the context to keep when evaluating the input value. All other parts are nullified.
	features_to_keep: ContextFeatures,
) -> T {
	let new_context = OwnedContextImpl::from_flags(ctx, features_to_keep);

	value.eval(Some(new_context.into())).await
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::graphene_hash::CacheHash;
	use core_types::transform::Footprint;
	use std::collections::hash_map::DefaultHasher;
	use std::hash::Hasher;

	/// Verifies that nullified context fields don't affect the cache hash — only the kept features matter.
	#[test]
	fn test_nullified_context_hash_stability() {
		use core_types::Context;
		use std::sync::Arc;

		let original_ctx: Context = Some(Arc::new(
			OwnedContextImpl::empty()
				.with_footprint(Footprint::default())
				.with_index(1)
				.with_real_time(10.5)
				.with_vararg(Box::new("test"))
				.with_animation_time(20.25),
		));

		// A second context with different values for the nullified fields
		let changed_ctx: Context = Some(Arc::new(
			OwnedContextImpl::empty()
				.with_footprint(Footprint::default())
				.with_index(2)
				.with_real_time(999.9)
				.with_vararg(Box::new("test"))
				.with_animation_time(888.8),
		));

		// Nullify everything — both should hash the same regardless of their field values
		let features_to_keep = ContextFeatures::empty();
		let nullified1 = OwnedContextImpl::from_flags(original_ctx.clone().unwrap(), features_to_keep);
		let nullified2 = OwnedContextImpl::from_flags(changed_ctx.clone().unwrap(), features_to_keep);

		let mut hasher1 = DefaultHasher::new();
		nullified1.cache_hash(&mut hasher1);

		let mut hasher2 = DefaultHasher::new();
		nullified2.cache_hash(&mut hasher2);

		assert_eq!(
			hasher1.finish(),
			hasher2.finish(),
			"Hash of nullified context should remain stable regardless of input changes when features are nullified"
		);

		// Keep only footprint and varargs — both have the same footprint and vararg, so hash should still match
		let partial_features = ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS;
		let partial1 = OwnedContextImpl::from_flags(original_ctx.clone().unwrap(), partial_features);
		let partial2 = OwnedContextImpl::from_flags(changed_ctx.clone().unwrap(), partial_features);

		let mut hasher3 = DefaultHasher::new();
		partial1.cache_hash(&mut hasher3);

		let mut hasher4 = DefaultHasher::new();
		partial2.cache_hash(&mut hasher4);

		assert_eq!(
			hasher3.finish(),
			hasher4.finish(),
			"Hash should be stable when keeping only footprint and varargs and their values are the same"
		);
	}
}
