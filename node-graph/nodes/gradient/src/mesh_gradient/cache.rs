use std::hash::Hasher;

use core_types::{CacheHash, transform::Footprint};
use glam::DAffine2;
use graphene_cache::{Cache, GenerationalEviction, Lru};
use raster_types::TextureWeakRef;
use vector_types::{GradientInterpolation, GradientSpace, MeshGradient};

#[derive(Copy, Clone, PartialEq)]
pub struct EvaluatorKey(pub u64);

pub type MeshGradientEvaluatorCache = Cache<EvaluatorKey, Lru<4>>;
pub type MeshGradientOutputCache = Cache<Footprint, GenerationalEviction<2, 3>>;

#[derive(Copy, Clone, PartialEq)]
struct MeshToTargetKey(u64);

#[derive(Copy, Clone, PartialEq)]
pub(crate) struct TextureKey {
	evaluator: EvaluatorKey,
	mesh_to_target: MeshToTargetKey,
}

#[derive(Clone)]
pub(crate) struct CachedTexture {
	pub(crate) key: TextureKey,
	pub(crate) transform: DAffine2,
	pub(crate) texture: TextureWeakRef,
}

pub(crate) fn texture_key(evaluator_key: EvaluatorKey, mesh_to_target: DAffine2) -> TextureKey {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	mesh_to_target.cache_hash(&mut hasher);
	let mesh_to_target_key = MeshToTargetKey(hasher.finish());
	TextureKey {
		evaluator: evaluator_key,
		mesh_to_target: mesh_to_target_key,
	}
}

pub(crate) fn evaluator_cache_key(mesh_gradient: &MeshGradient, space: GradientSpace, interpolation: GradientInterpolation) -> EvaluatorKey {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	mesh_gradient.cache_hash(&mut hasher);
	space.cache_hash(&mut hasher);
	interpolation.cache_hash(&mut hasher);
	EvaluatorKey(hasher.finish())
}
