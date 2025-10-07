use crate::Artboard;
use crate::context::{CloneVarArgs, Context, ContextFeatures, Ctx, ExtractAll};
use crate::gradient::GradientStops;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::Table;
use crate::transform::Footprint;
use crate::uuid::NodeId;
use crate::vector::Vector;
use crate::{Graphic, OwnedContextImpl};
use core::f64;
use glam::{DAffine2, DVec2};
use graphene_core_shaders::color::Color;

/// Node for filtering components of the context based on the specified requirements.
/// This node is inserted by the compiler to "zero out" unused context components.
#[node_macro::node(category("Internal"))]
async fn context_modification<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
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
		Context -> Vec<DVec2>,
		Context -> Vec<NodeId>,
		Context -> Vec<f64>,
		Context -> Vec<f32>,
		Context -> Table<Vector>,
		Context -> Table<Graphic>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> Table<Artboard>,
		Context -> Table<GradientStops>,
		Context -> GradientStops,
	)]
	value: impl Node<Context<'static>, Output = T>,
	features_to_keep: ContextFeatures,
) -> T {
	let new_context = OwnedContextImpl::from_flags(ctx, features_to_keep);

	value.eval(Some(new_context.into())).await
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::transform::Footprint;
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};

	/// Test that the hash of a nullified context remains stable even when nullified inputs change
	#[test]
	fn test_nullified_context_hash_stability() {
		use crate::Context;
		use std::sync::Arc;

		// Create original contexts using the Context type (Option<Arc<OwnedContextImpl>>)
		let original_ctx: Context = Some(Arc::new(
			OwnedContextImpl::empty()
				.with_footprint(Footprint::default())
				.with_index(1)
				.with_real_time(10.5)
				.with_vararg(Box::new("test"))
				.with_animation_time(20.25),
		));

		// Test nullifying different features - hash should remain stable for each nullification
		let features_to_keep = ContextFeatures::empty(); // Nullify everything

		// Create nullified context - this should only keep features specified in features_to_keep
		let nullified_ctx = OwnedContextImpl::from_flags(original_ctx.clone().unwrap(), features_to_keep);

		// Calculate hash of nullified context
		let mut hasher1 = DefaultHasher::new();
		nullified_ctx.hash(&mut hasher1);
		let hash1 = hasher1.finish();

		// Create a different original context with changed values
		let changed_ctx: Context = Some(Arc::new(
			OwnedContextImpl::empty()
				.with_footprint(Footprint::default()) // Same footprint
				.with_index(2)
				.with_real_time(999.9) // Different real time
				.with_vararg(Box::new("test"))
				.with_animation_time(888.8), // Different animation time
		));

		// Create nullified context from the changed original - should have same hash since everything is nullified
		let nullified_changed_ctx = OwnedContextImpl::from_flags(changed_ctx.clone().unwrap(), features_to_keep);

		let mut hasher2 = DefaultHasher::new();
		nullified_changed_ctx.hash(&mut hasher2);
		let hash2 = hasher2.finish();

		// Hash should be the same because all features were nullified
		assert_eq!(hash1, hash2, "Hash of nullified context should remain stable regardless of input changes when features are nullified");

		// Test partial nullification - keep only footprint
		let partial_features = ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS;

		let partial_nullified1 = OwnedContextImpl::from_flags(original_ctx.clone().unwrap(), partial_features);
		let partial_nullified2 = OwnedContextImpl::from_flags(changed_ctx.clone().unwrap(), partial_features);

		let mut hasher3 = DefaultHasher::new();
		partial_nullified1.hash(&mut hasher3);
		let hash3 = hasher3.finish();

		let mut hasher4 = DefaultHasher::new();
		partial_nullified2.hash(&mut hasher4);
		let hash4 = hasher4.finish();

		// These should be the same because both have the same footprint (Footprint::default()) and varargs
		// and other features are nullified
		assert_eq!(hash3, hash4, "Hash should be stable when keeping only footprint and footprint values are the same");
	}
}
