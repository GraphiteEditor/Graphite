use core::f64;
use core_types::context::{CloneVarArgs, Context, ContextModification, Ctx, ExtractAll};
use core_types::list::{AttributeValueDyn, Item, List, ListDyn, NodeIdPath};
use core_types::transform::Footprint;
use core_types::{Color, Interrupt, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphene_animation::AnimationCurve;
use graphic_types::Generator;
use graphic_types::vector_types::Gradient;
use graphic_types::{Artboard, Graphic, Vector};
use raster_types::{CPU, GPU, Raster};

/// Filters out what should be unused components of the context based on the specified requirements.
/// This node is inserted by the compiler to "zero out" unused context components.
#[node_macro::node(category("_Context Nullification"), forwards_input)]
fn context_modification<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	/// The data to pass through, evaluated with the stripped down context.
	#[implementations(
		Context -> Item<bool>,
		Context -> Item<i64>,
		Context -> Item<f64>,
		Context -> Item<String>,
		Context -> Item<DAffine2>,
		Context -> Item<Footprint>,
		Context -> Item<DVec2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Artboard>,
		Context -> Item<Gradient>,
		Context -> Item<Generator>,
		Context -> Item<NodeIdPath>,
		Context -> Item<AttributeValueDyn>,
		Context -> Item<AnimationCurve>,
		Context -> List<String>,
		Context -> List<f64>,
		Context -> List<DVec2>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Artboard>,
		Context -> List<Gradient>,
		Context -> List<Generator>,
		Context -> List<AnimationCurve>,
		Context -> ListDyn,
	)]
	value: impl Node<Context<'static>, Output = T>,
	/// The parts of the context to keep when evaluating the input value. All other parts are nullified.
	modification: Item<ContextModification>,
) -> Result<T, Interrupt> {
	let new_context = OwnedContextImpl::from_modification(ctx, modification.element());

	value.eval(Some(new_context.into()))
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::context::ContextFeatures;
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

	fn hash_of(context: OwnedContextImpl) -> u64 {
		let mut hasher = DefaultHasher::new();
		context.cache_hash(&mut hasher);
		hasher.finish()
	}

	fn with_generations(generations: &[(u64, u64)]) -> std::sync::Arc<OwnedContextImpl> {
		let generations = generations.iter().map(|&(source, generation)| (SourceId(source), generation)).collect();
		std::sync::Arc::new(OwnedContextImpl::empty().with_generations(generations))
	}

	#[test]
	fn a_modification_keys_only_on_its_retained_sources() {
		let modification = ContextModification {
			features: ContextFeatures::all(),
			sources: [SourceId(1)].into(),
		};
		let key = |generations: &[(u64, u64)]| hash_of(OwnedContextImpl::from_modification(with_generations(generations), &modification));
		let initial = key(&[(0, 1), (1, 3)]);
		assert_eq!(initial, key(&[(0, 2), (1, 3)]), "a bump of an unretained source keeps the key");
		assert_ne!(initial, key(&[(0, 1), (1, 4)]), "a bump of a retained source changes the key");
	}

	#[test]
	fn a_source_key_ignores_its_own_bumps() {
		let key = |generations: &[(u64, u64)]| hash_of(OwnedContextImpl::from(with_generations(generations)).without_source(SourceId(7)));
		let initial = key(&[(7, 1), (9, 5)]);
		assert_eq!(initial, key(&[(7, 2), (9, 5)]), "its own bump keeps the key");
		assert_ne!(initial, key(&[(7, 1), (9, 6)]), "another source's bump changes the key");
	}

	#[test]
	fn an_unfiltered_context_sees_every_bump() {
		let key = |generations: &[(u64, u64)]| hash_of(OwnedContextImpl::from(with_generations(generations)));
		assert_ne!(key(&[(0, 1)]), key(&[(0, 2)]));
	}
}
