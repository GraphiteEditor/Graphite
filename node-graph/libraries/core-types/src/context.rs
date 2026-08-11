use crate::arena::Arena;
use crate::transform::Footprint;
use glam::DVec2;
pub use no_std_types::context::{ArcCtx, Ctx};
use std::any::Any;
use std::hash::{Hash, Hasher};
use std::panic::Location;
use std::sync::Arc;

// ==============
// EXTRACT TRAITS
// ==============

pub trait ExtractFootprint {
	#[track_caller]
	fn try_footprint(&self) -> Option<&Footprint>;
	#[track_caller]
	fn footprint(&self) -> &Footprint {
		self.try_footprint().unwrap_or_else(|| {
			log::error!("Context did not have a footprint, called from: {}", Location::caller());
			&Footprint::DEFAULT
		})
	}
}
pub trait ExtractRealTime {
	fn try_real_time(&self) -> Option<f64>;
}
pub trait ExtractAnimationTime {
	fn try_animation_time(&self) -> Option<f64>;
}
pub trait ExtractPointerPosition {
	fn try_pointer_position(&self) -> Option<DVec2>;
}
pub trait ExtractPosition {
	fn try_position(&self) -> Option<impl Iterator<Item = DVec2>>;
}
pub trait ExtractIndex {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>>;
	fn innermost_index(&self) -> u64 {
		self.try_index().and_then(|mut indices| indices.next()).unwrap_or(0) as u64
	}
}
pub trait ExtractVarArgs {
	// TODO: Consider returning a slice or something like that

	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult>;
	fn varargs_len(&self) -> Result<usize, VarArgsResult>;
	fn hash_varargs(&self, hasher: &mut dyn Hasher);
}

pub trait CloneVarArgs: ExtractVarArgs {
	// TODO: Consider returning a slice or something like that

	// fn box_clone(&self) -> Vec<DynBox>;
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>>;
}

// =============
// INJECT TRAITS
// =============

// Inject* traits for providing context features to downstream nodes
pub trait InjectFootprint {}
pub trait InjectRealTime {}
pub trait InjectAnimationTime {}
pub trait InjectPointerPosition {}
pub trait InjectPosition {}
pub trait InjectIndex {
	fn set_index(&mut self, index: u64);
}
pub trait InjectVarArgs {}

// ================
// EXTRACTALL TRAIT
// ================

pub trait ExtractAll:
	// Extract traits
	ExtractFootprint +
	ExtractRealTime +
	ExtractAnimationTime +
	ExtractPointerPosition +
	ExtractPosition +
	ExtractIndex +
	ExtractVarArgs {}
impl<
	T: ?Sized
		// Extract traits
		+ ExtractFootprint
		+ ExtractRealTime
		+ ExtractAnimationTime
		+ ExtractPointerPosition
		+ ExtractPosition
		+ ExtractIndex
		+ ExtractVarArgs,
> ExtractAll for T
{
}

// =============
// INJECT TRAITS
// =============

impl<T: Ctx> InjectFootprint for T {}
impl<T: Ctx> InjectRealTime for T {}
impl<T: Ctx> InjectAnimationTime for T {}
impl<T: Ctx> InjectPointerPosition for T {}
impl<T: Ctx> InjectPosition for T {}
impl<T: Ctx> InjectVarArgs for T {}

// =============
// MODIFY TRAITS
// =============

// Modify* marker traits for context-transparent nodes
pub trait ModifyFootprint: ExtractFootprint + InjectFootprint {}
pub trait ModifyRealTime: ExtractRealTime + InjectRealTime {}
pub trait ModifyAnimationTime: ExtractAnimationTime + InjectAnimationTime {}
pub trait ModifyPointerPosition: ExtractPointerPosition + InjectPointerPosition {}
pub trait ModifyPosition: ExtractPosition + InjectPosition {}
pub trait ModifyIndex: ExtractIndex + InjectIndex {}
pub trait ModifyVarArgs: ExtractVarArgs + InjectVarArgs {}

impl<T: Ctx + InjectFootprint + ExtractFootprint> ModifyFootprint for T {}
impl<T: Ctx + InjectRealTime + ExtractRealTime> ModifyRealTime for T {}
impl<T: Ctx + InjectAnimationTime + ExtractAnimationTime> ModifyAnimationTime for T {}
impl<T: Ctx + InjectPointerPosition + ExtractPointerPosition> ModifyPointerPosition for T {}
impl<T: Ctx + InjectPosition + ExtractPosition> ModifyPosition for T {}
impl<T: Ctx + InjectIndex + ExtractIndex> ModifyIndex for T {}
impl<T: Ctx + InjectVarArgs + ExtractVarArgs> ModifyVarArgs for T {}

// ================
// CONTEXT FEATURES
// ================

// Public enum for flexible node macro codegen
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ContextFeature {
	ExtractFootprint,
	ExtractRealTime,
	ExtractAnimationTime,
	ExtractPointerPosition,
	ExtractPosition,
	ExtractIndex,
	ExtractVarArgs,
	InjectFootprint,
	InjectRealTime,
	InjectAnimationTime,
	InjectPointerPosition,
	InjectPosition,
	InjectIndex,
	InjectVarArgs,
}

// Internal bitflags for fast compiler analysis
use bitflags::bitflags;
bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dyn_any::DynAny, Default)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct ContextFeatures: u32 {
		const FOOTPRINT = 1 << 0;
		const REAL_TIME = 1 << 1;
		const ANIMATION_TIME = 1 << 2;
		const POINTER_POSITION = 1 << 3;
		const POSITION = 1 << 4;
		const INDEX = 1 << 5;
		const VARARGS = 1 << 6;
	}
}

impl graphene_hash::CacheHash for ContextFeatures {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(self, state);
	}
}

impl ContextFeatures {
	pub fn name(&self) -> &'static str {
		match *self {
			ContextFeatures::FOOTPRINT => "Footprint",
			ContextFeatures::REAL_TIME => "RealTime",
			ContextFeatures::ANIMATION_TIME => "AnimationTime",
			ContextFeatures::POINTER_POSITION => "PointerPosition",
			ContextFeatures::POSITION => "Position",
			ContextFeatures::INDEX => "Index",
			ContextFeatures::VARARGS => "VarArgs",
			_ => "Multiple Features",
		}
	}
}

// ====================
// CONTEXT DEPENDENCIES
// ====================

#[derive(Debug, Clone, PartialEq, Eq, Hash, graphene_hash::CacheHash, dyn_any::DynAny, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextDependencies {
	pub extract: ContextFeatures,
	pub inject: ContextFeatures,
	#[cfg_attr(feature = "serde", serde(default, deserialize_with = "deserialize_sorted_sources"))]
	sources: Vec<SourceId>,
}

impl ContextDependencies {
	pub fn new(extract: ContextFeatures, inject: ContextFeatures) -> Self {
		Self { extract, inject, sources: Vec::new() }
	}

	pub fn sources(&self) -> &[SourceId] {
		&self.sources
	}

	pub fn add_sources(&mut self, sources: &[SourceId]) {
		merge_sorted_sources(&mut self.sources, sources);
	}

	pub fn from_sources(sources: &[SourceId]) -> Self {
		let mut dependencies = Self::default();
		dependencies.add_sources(sources);
		dependencies
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, graphene_hash::CacheHash, dyn_any::DynAny, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextModification {
	pub features: ContextFeatures,
	#[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_sorted_sources"))]
	sources: Vec<SourceId>,
}

impl ContextModification {
	pub fn sources(&self) -> &[SourceId] {
		&self.sources
	}

	pub fn add_sources(&mut self, sources: &[SourceId]) {
		merge_sorted_sources(&mut self.sources, sources);
	}

	pub fn from_sources(features: ContextFeatures, sources: &[SourceId]) -> Self {
		let mut modification = Self { features, sources: Vec::new() };
		modification.add_sources(sources);
		modification
	}
}

/// Restores the sorted-and-deduplicated invariant that `contains` and `difference`
/// rely on for binary search, which arbitrary serialized input can violate.
#[cfg(feature = "serde")]
fn deserialize_sorted_sources<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<SourceId>, D::Error> {
	use serde::Deserialize;
	let mut sources = Vec::deserialize(deserializer)?;
	sources.sort_unstable();
	sources.dedup();
	Ok(sources)
}

impl core::ops::BitOrAssign<&ContextModification> for ContextModification {
	fn bitor_assign(&mut self, other: &Self) {
		self.features |= other.features;
		merge_sorted_sources(&mut self.sources, &other.sources);
	}
}

impl core::ops::BitOrAssign for ContextModification {
	fn bitor_assign(&mut self, other: Self) {
		*self |= &other;
	}
}

impl core::ops::BitOrAssign<ContextFeatures> for ContextModification {
	fn bitor_assign(&mut self, features: ContextFeatures) {
		self.features |= features;
	}
}

impl core::ops::BitAndAssign<ContextFeatures> for ContextModification {
	fn bitand_assign(&mut self, features: ContextFeatures) {
		self.features &= features;
	}
}

impl ContextModification {
	pub fn contains(&self, other: &Self) -> bool {
		debug_assert!(self.sources.is_sorted() && other.sources.is_sorted());
		self.features.contains(other.features) && other.sources.iter().all(|id| self.sources.binary_search(id).is_ok())
	}

	pub fn difference(&self, other: &Self) -> Self {
		debug_assert!(other.sources.is_sorted());
		Self {
			features: self.features.difference(other.features),
			sources: self.sources.iter().copied().filter(|id| other.sources.binary_search(id).is_err()).collect(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.features.is_empty() && self.sources.is_empty()
	}
}

/// Sorts and deduplicates unconditionally, so the result is ordered no matter what
/// order the inputs arrived in.
pub fn merge_sorted_sources(sources: &mut Vec<SourceId>, other: &[SourceId]) {
	sources.extend_from_slice(other);
	sources.sort_unstable();
	sources.dedup();
}

impl From<&[ContextFeature]> for ContextDependencies {
	fn from(features: &[ContextFeature]) -> Self {
		let mut extract = ContextFeatures::empty();
		let mut inject = ContextFeatures::empty();
		for feature in features {
			extract |= match feature {
				ContextFeature::ExtractFootprint => ContextFeatures::FOOTPRINT,
				ContextFeature::ExtractRealTime => ContextFeatures::REAL_TIME,
				ContextFeature::ExtractAnimationTime => ContextFeatures::ANIMATION_TIME,
				ContextFeature::ExtractPointerPosition => ContextFeatures::POINTER_POSITION,
				ContextFeature::ExtractPosition => ContextFeatures::POSITION,
				ContextFeature::ExtractIndex => ContextFeatures::INDEX,
				ContextFeature::ExtractVarArgs => ContextFeatures::VARARGS,
				_ => ContextFeatures::empty(),
			};
			inject |= match feature {
				ContextFeature::InjectFootprint => ContextFeatures::FOOTPRINT,
				ContextFeature::InjectRealTime => ContextFeatures::REAL_TIME,
				ContextFeature::InjectAnimationTime => ContextFeatures::ANIMATION_TIME,
				ContextFeature::InjectPointerPosition => ContextFeatures::POINTER_POSITION,
				ContextFeature::InjectPosition => ContextFeatures::POSITION,
				ContextFeature::InjectIndex => ContextFeatures::INDEX,
				ContextFeature::InjectVarArgs => ContextFeatures::VARARGS,
				_ => ContextFeatures::empty(),
			};
		}
		Self { extract, inject, sources: Vec::new() }
	}
}

// ===================================
// EXTRACT TRAIT IMPLS FOR `Option<T>`
// ===================================

impl<T: ExtractFootprint + Sync> ExtractFootprint for Option<T> {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.as_ref().and_then(|x| x.try_footprint())
	}
	#[track_caller]
	fn footprint(&self) -> &Footprint {
		self.try_footprint().unwrap_or_else(|| {
			log::warn!("trying to extract footprint from context None {} ", Location::caller());
			&Footprint::DEFAULT
		})
	}
}
impl<T: ExtractRealTime + Sync> ExtractRealTime for Option<T> {
	fn try_real_time(&self) -> Option<f64> {
		self.as_ref().and_then(|x| x.try_real_time())
	}
}
impl<T: ExtractAnimationTime + Sync> ExtractAnimationTime for Option<T> {
	fn try_animation_time(&self) -> Option<f64> {
		self.as_ref().and_then(|x| x.try_animation_time())
	}
}
impl<T: ExtractPointerPosition + Sync> ExtractPointerPosition for Option<T> {
	fn try_pointer_position(&self) -> Option<DVec2> {
		self.as_ref().and_then(|x| x.try_pointer_position())
	}
}
impl<T: ExtractPosition + Sync> ExtractPosition for Option<T> {
	fn try_position(&self) -> Option<impl Iterator<Item = DVec2>> {
		self.as_ref().and_then(|x| x.try_position())
	}
}
impl<T: ExtractIndex> ExtractIndex for Option<T> {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>> {
		self.as_ref().and_then(|x| x.try_index())
	}
}
impl<T: ExtractVarArgs + Sync> ExtractVarArgs for Option<T> {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		let Some(inner) = self else { return Err(VarArgsResult::NoVarArgs) };
		inner.vararg(index)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		let Some(inner) = self else { return Err(VarArgsResult::NoVarArgs) };
		inner.varargs_len()
	}

	fn hash_varargs(&self, hasher: &mut dyn Hasher) {
		if let Some(inner) = self {
			inner.hash_varargs(hasher)
		}
	}
}

impl<T: CloneVarArgs + Sync> CloneVarArgs for Option<T> {
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>> {
		self.as_ref().and_then(CloneVarArgs::arc_clone)
	}
}

// ================================
// EXTRACT TRAIT IMPLS FOR `Arc<T>`
// ================================

impl<T: ExtractFootprint + Sync> ExtractFootprint for Arc<T> {
	fn try_footprint(&self) -> Option<&Footprint> {
		(**self).try_footprint()
	}
}
impl<T: ExtractRealTime + Sync> ExtractRealTime for Arc<T> {
	fn try_real_time(&self) -> Option<f64> {
		(**self).try_real_time()
	}
}
impl<T: ExtractAnimationTime + Sync> ExtractAnimationTime for Arc<T> {
	fn try_animation_time(&self) -> Option<f64> {
		(**self).try_animation_time()
	}
}
impl<T: ExtractPointerPosition + Sync> ExtractPointerPosition for Arc<T> {
	fn try_pointer_position(&self) -> Option<DVec2> {
		(**self).try_pointer_position()
	}
}
impl<T: ExtractPosition + Sync> ExtractPosition for Arc<T> {
	fn try_position(&self) -> Option<impl Iterator<Item = DVec2>> {
		(**self).try_position()
	}
}
impl<T: ExtractIndex> ExtractIndex for Arc<T> {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>> {
		(**self).try_index()
	}
}
impl<T: ExtractVarArgs + Sync> ExtractVarArgs for Arc<T> {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		(**self).vararg(index)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		(**self).varargs_len()
	}

	fn hash_varargs(&self, hasher: &mut dyn Hasher) {
		(**self).hash_varargs(hasher)
	}
}

impl<T: CloneVarArgs + Sync> CloneVarArgs for Arc<T> {
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>> {
		(**self).arc_clone()
	}
}

// ============================
// EXTRACT TRAIT IMPLS FOR `&T`
// ============================

impl<T: ExtractFootprint + Ctx + Sync + Send> ExtractFootprint for &T {
	fn try_footprint(&self) -> Option<&Footprint> {
		(*self).try_footprint()
	}
}

impl<T: ExtractVarArgs + Sync> ExtractVarArgs for &T {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		(*self).vararg(index)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		(*self).varargs_len()
	}

	fn hash_varargs(&self, hasher: &mut dyn Hasher) {
		(*self).hash_varargs(hasher)
	}
}

// ============================
// EXTRACT TRAIT IMPLS FOR `()`
// ============================

impl Ctx for Footprint {}

impl ExtractFootprint for () {
	fn try_footprint(&self) -> Option<&Footprint> {
		log::error!("tried to extract footprint form (), {}", Location::caller());
		None
	}
}

// ==============
// TYPE `Context`
// ==============

pub type Context<'a> = ContextImpl<'a>;
type DynRef<'a> = &'a (dyn Any + Send + Sync);

pub trait DynHash {
	fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<H: graphene_hash::CacheHash + ?Sized> DynHash for H {
	fn dyn_hash(&self, mut state: &mut dyn Hasher) {
		graphene_hash::CacheHash::cache_hash(self, &mut state);
	}
}

impl Hash for dyn AnyHash {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.dyn_hash(state);
	}
}
impl Hash for Box<dyn AnyHash + Send + Sync> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		(**self).dyn_hash(state);
	}
}

pub trait AnyHash: DynHash + Any {}
impl<T: DynHash + Any> AnyHash for T {}

pub trait VarArg: AnyHash {
	fn clone_slot(&self) -> OwnedSlot;
}

impl<T: AnyHash + Clone + Send + Sync> VarArg for T {
	fn clone_slot(&self) -> OwnedSlot {
		OwnedSlot(Box::new(self.clone()))
	}
}

pub struct OwnedSlot(Box<dyn VarArg + Send + Sync>);

impl Clone for OwnedSlot {
	fn clone(&self) -> Self {
		self.0.clone_slot()
	}
}

impl std::ops::Deref for OwnedSlot {
	type Target = dyn VarArg + Send + Sync;

	fn deref(&self) -> &Self::Target {
		self.0.as_ref()
	}
}

impl std::fmt::Debug for OwnedSlot {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("OwnedSlot")
	}
}

pub type SourceId = u64;

#[derive(Clone, Copy, Debug)]
pub struct IndexLink<'a> {
	pub index: u64,
	pub outer: Option<&'a IndexLink<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub struct PositionLink<'a> {
	pub position: DVec2,
	pub outer: Option<&'a PositionLink<'a>>,
}

pub type DynSlot<'a> = &'a (dyn VarArg + Send + Sync);

#[derive(Clone, Copy)]
pub enum VarArgSlots<'a> {
	Single(DynSlot<'a>),
	Slice(&'a [DynSlot<'a>]),
}

impl<'a> VarArgSlots<'a> {
	pub fn get(&self, index: usize) -> Option<DynSlot<'a>> {
		match self {
			VarArgSlots::Single(slot) => (index == 0).then_some(*slot),
			VarArgSlots::Slice(slots) => slots.get(index).copied(),
		}
	}

	pub fn len(&self) -> usize {
		match self {
			VarArgSlots::Single(_) => 1,
			VarArgSlots::Slice(slots) => slots.len(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn iter(&self) -> impl Iterator<Item = DynSlot<'a>> + '_ {
		(0..self.len()).filter_map(move |index| self.get(index))
	}
}

#[derive(Clone, Copy)]
pub struct VarArgLink<'a> {
	pub args: VarArgSlots<'a>,
	pub outer: Option<&'a VarArgLink<'a>>,
}

impl<'a> std::fmt::Debug for VarArgLink<'a> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("VarArgLink").field("args_len", &self.args.len()).field("outer", &self.outer).finish()
	}
}

#[derive(Clone, Copy, Debug)]
pub struct EvalScope<'a> {
	real_time: Option<f64>,
	animation_time: Option<f64>,
	pointer_position: Option<DVec2>,
	generations: &'a [(SourceId, u64)],
	arena: &'a Arena,
	hash: u64,
}

impl<'a> EvalScope<'a> {
	pub fn new(real_time: Option<f64>, animation_time: Option<f64>, pointer_position: Option<DVec2>, generations: &'a [(SourceId, u64)], arena: &'a Arena) -> Self {
		let mut scope = Self {
			real_time,
			animation_time,
			pointer_position,
			generations,
			arena,
			hash: 0,
		};
		scope.hash = scope.compute_hash(|_| true);
		scope
	}

	pub fn with_real_time(&self, real_time: Option<f64>) -> EvalScope<'a> {
		let mut scope = EvalScope { real_time, ..*self };
		scope.hash = scope.compute_hash(|_| true);
		scope
	}

	pub fn with_animation_time(&self, animation_time: Option<f64>) -> EvalScope<'a> {
		let mut scope = EvalScope { animation_time, ..*self };
		scope.hash = scope.compute_hash(|_| true);
		scope
	}

	pub fn with_pointer_position(&self, pointer_position: Option<DVec2>) -> EvalScope<'a> {
		let mut scope = EvalScope { pointer_position, ..*self };
		scope.hash = scope.compute_hash(|_| true);
		scope
	}

	pub fn nullified(&self, keep: ContextFeatures, retain: Option<&[SourceId]>) -> EvalScope<'a> {
		let mut scope = EvalScope {
			real_time: self.real_time.filter(|_| keep.contains(ContextFeatures::REAL_TIME)),
			animation_time: self.animation_time.filter(|_| keep.contains(ContextFeatures::ANIMATION_TIME)),
			pointer_position: self.pointer_position.filter(|_| keep.contains(ContextFeatures::POINTER_POSITION)),
			..*self
		};
		scope.hash = scope.compute_hash(|source| retain.is_none_or(|retain| retain.contains(source)));
		scope
	}

	pub fn excluding(&self, source: SourceId) -> EvalScope<'a> {
		let mut scope = *self;
		scope.hash = scope.compute_hash(|candidate| *candidate != source);
		scope
	}

	fn compute_hash(&self, keep_source: impl Fn(&SourceId) -> bool) -> u64 {
		let mut hasher = std::hash::DefaultHasher::new();
		self.real_time.map(f64::to_bits).hash(&mut hasher);
		self.animation_time.map(f64::to_bits).hash(&mut hasher);
		self.pointer_position.map(|position| (position.x.to_bits(), position.y.to_bits())).hash(&mut hasher);
		for (source, generation) in self.generations {
			if keep_source(source) {
				(source, generation).hash(&mut hasher);
			}
		}
		hasher.finish()
	}

	pub fn generation(&self, source: SourceId) -> Option<u64> {
		self.generations.iter().find(|(candidate, _)| *candidate == source).map(|(_, generation)| *generation)
	}

	pub fn generations(&self) -> &'a [(SourceId, u64)] {
		self.generations
	}

	pub fn arena(&self) -> &'a Arena {
		self.arena
	}
}

pub trait ExtractArena {
	type ArenaRef;
	fn arena(&self) -> Self::ArenaRef;
}

pub trait CtxFamily {
	type Ctx<'s>: Ctx + DeriveCtx<Family = Self>;
}

pub type Derived<'s, C> = <<C as DeriveCtx>::Family as CtxFamily>::Ctx<'s>;

pub trait DeriveCtx {
	type Family: CtxFamily;
	fn derived(&self) -> Derived<'_, Self>;
	fn index_head(&self) -> IndexLink<'_>;
	fn scope(&self) -> &EvalScope<'_>;
	fn position_head(&self) -> Option<&PositionLink<'_>>;
	fn varargs_head(&self) -> Option<&VarArgLink<'_>>;
	fn promoted<'s>(&'s self, spilled_head: &'s IndexLink<'s>, inner_index: u64) -> Derived<'s, Self>;
	fn with_footprint<'s>(&'s self, footprint: &'s Footprint) -> Derived<'s, Self>;
	fn with_varargs<'s>(&'s self, varargs: &'s VarArgLink<'s>) -> Derived<'s, Self>;
	fn with_position<'s>(&'s self, position: &'s PositionLink<'s>) -> Derived<'s, Self>;
	fn with_scope<'s>(&'s self, scope: &'s EvalScope<'s>) -> Derived<'s, Self>;
	fn nullified<'s>(&'s self, keep: ContextFeatures, scope: &'s EvalScope<'s>) -> Derived<'s, Self>;

	fn modify_footprint(&self, modify: impl FnOnce(&mut Footprint)) -> ModifiedFootprint<'_, Self>
	where
		Self: ExtractFootprint + Sized,
	{
		let mut footprint = self.try_footprint().copied();
		if let Some(footprint) = &mut footprint {
			modify(footprint);
		}
		ModifiedFootprint { ctx: self, footprint }
	}

	fn push_vararg<'s>(&'s self, arg: DynSlot<'s>) -> VarArgScope<'s, Self>
	where
		Self: Sized,
	{
		VarArgScope {
			ctx: self,
			link: VarArgLink {
				args: VarArgSlots::Single(arg),
				outer: self.varargs_head(),
			},
		}
	}

	fn push_position(&self, position: DVec2) -> PositionScope<'_, Self>
	where
		Self: Sized,
	{
		PositionScope {
			ctx: self,
			link: PositionLink {
				position,
				outer: self.position_head(),
			},
		}
	}
}

pub struct PositionScope<'c, C> {
	ctx: &'c C,
	link: PositionLink<'c>,
}

impl<C: DeriveCtx> PositionScope<'_, C> {
	pub fn ctx(&self) -> Derived<'_, C> {
		self.ctx.with_position(&self.link)
	}
}

#[derive(Clone, Debug, Default)]
pub struct CtxSnapshot {
	footprint: Option<Footprint>,
	real_time: Option<f64>,
	animation_time: Option<f64>,
	pointer_position: Option<DVec2>,
	index: Option<Vec<usize>>,
	positions: Option<Vec<DVec2>>,
	varargs: Vec<Vec<OwnedSlot>>,
	generations: Vec<(SourceId, u64)>,
}

impl CtxSnapshot {
	pub fn capture<C>(ctx: &C) -> Self
	where
		C: DeriveCtx + ExtractFootprint + ExtractRealTime + ExtractAnimationTime + ExtractPointerPosition + ExtractIndex + ExtractPosition,
	{
		Self {
			footprint: ctx.try_footprint().copied(),
			real_time: ctx.try_real_time(),
			animation_time: ctx.try_animation_time(),
			pointer_position: ctx.try_pointer_position(),
			index: ctx.try_index().map(|levels| levels.collect()),
			positions: ctx.try_position().map(|positions| positions.collect()),
			varargs: std::iter::successors(ctx.varargs_head(), |link| link.outer)
				.map(|link| link.args.iter().map(|slot| slot.clone_slot()).collect())
				.collect(),
			generations: ctx.scope().generations().to_vec(),
		}
	}

	pub fn generations(&self) -> &[(SourceId, u64)] {
		&self.generations
	}
}

impl ExtractFootprint for CtxSnapshot {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.footprint.as_ref()
	}
}

impl ExtractRealTime for CtxSnapshot {
	fn try_real_time(&self) -> Option<f64> {
		self.real_time
	}
}

impl ExtractAnimationTime for CtxSnapshot {
	fn try_animation_time(&self) -> Option<f64> {
		self.animation_time
	}
}

impl ExtractPointerPosition for CtxSnapshot {
	fn try_pointer_position(&self) -> Option<DVec2> {
		self.pointer_position
	}
}

impl ExtractIndex for CtxSnapshot {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>> {
		self.index.as_ref().map(|levels| levels.iter().copied())
	}
}

impl ExtractPosition for CtxSnapshot {
	fn try_position(&self) -> Option<impl Iterator<Item = DVec2>> {
		self.positions.as_ref().map(|positions| positions.iter().copied())
	}
}

impl ExtractVarArgs for CtxSnapshot {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		if self.varargs.is_empty() {
			return Err(VarArgsResult::NoVarArgs);
		}
		let slot = self.varargs.iter().flatten().nth(index).ok_or(VarArgsResult::IndexOutOfBounds)?;
		Ok(&**slot as DynRef<'_>)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		if self.varargs.is_empty() {
			return Err(VarArgsResult::NoVarArgs);
		}
		Ok(self.varargs.iter().map(|level| level.len()).sum())
	}

	fn hash_varargs(&self, hasher: &mut dyn Hasher) {
		let mut count = 0u64;
		for slot in self.varargs.iter().flatten() {
			slot.dyn_hash(&mut *hasher);
			count += 1;
		}
		count.hash(&mut &mut *hasher);
	}
}

pub struct VarArgScope<'c, C> {
	ctx: &'c C,
	link: VarArgLink<'c>,
}

impl<C: DeriveCtx> VarArgScope<'_, C> {
	pub fn ctx(&self) -> Derived<'_, C> {
		self.ctx.with_varargs(&self.link)
	}
}

pub struct ModifiedFootprint<'c, C> {
	ctx: &'c C,
	footprint: Option<Footprint>,
}

impl<C: DeriveCtx> ModifiedFootprint<'_, C> {
	pub fn ctx(&self) -> Derived<'_, C> {
		match &self.footprint {
			Some(footprint) => self.ctx.with_footprint(footprint),
			None => self.ctx.derived(),
		}
	}
}

pub struct ContextImplFamily;

impl CtxFamily for ContextImplFamily {
	type Ctx<'s> = ContextImpl<'s>;
}

#[derive(Clone, Copy, Debug)]
pub struct ContextImpl<'a> {
	index: IndexLink<'a>,
	position: Option<&'a PositionLink<'a>>,
	varargs: Option<&'a VarArgLink<'a>>,
	footprint: Option<&'a Footprint>,
	scope: &'a EvalScope<'a>,
}

impl<'a> ContextImpl<'a> {
	pub fn root(scope: &'a EvalScope<'a>) -> Self {
		Self {
			index: IndexLink { index: 0, outer: None },
			position: None,
			varargs: None,
			footprint: None,
			scope,
		}
	}

	pub fn scope(&self) -> &'a EvalScope<'a> {
		self.scope
	}

	pub fn index_head(&self) -> IndexLink<'a> {
		self.index
	}

	pub fn with_footprint<'s>(&self, footprint: &'s Footprint) -> ContextImpl<'s>
	where
		'a: 's,
	{
		ContextImpl { footprint: Some(footprint), ..*self }
	}

	pub fn with_scope<'s>(&self, scope: &'s EvalScope<'s>) -> ContextImpl<'s>
	where
		'a: 's,
	{
		ContextImpl { scope, ..*self }
	}

	pub fn with_varargs<'s>(&self, varargs: &'s VarArgLink<'s>) -> ContextImpl<'s>
	where
		'a: 's,
	{
		ContextImpl { varargs: Some(varargs), ..*self }
	}

	pub fn with_position<'s>(&self, position: &'s PositionLink<'s>) -> ContextImpl<'s>
	where
		'a: 's,
	{
		ContextImpl { position: Some(position), ..*self }
	}

	pub fn nullified<'s>(&self, keep: ContextFeatures, scope: &'s EvalScope<'s>) -> ContextImpl<'s>
	where
		'a: 's,
	{
		ContextImpl {
			index: match keep.contains(ContextFeatures::INDEX) {
				true => self.index,
				false => IndexLink { index: 0, outer: None },
			},
			position: self.position.filter(|_| keep.contains(ContextFeatures::POSITION)),
			varargs: self.varargs.filter(|_| keep.contains(ContextFeatures::VARARGS)),
			footprint: self.footprint.filter(|_| keep.contains(ContextFeatures::FOOTPRINT)),
			scope,
		}
	}

	pub fn promoted<'s>(&self, spilled_head: &'s IndexLink<'s>, inner_index: u64) -> ContextImpl<'s>
	where
		'a: 's,
	{
		ContextImpl {
			index: IndexLink {
				index: inner_index,
				outer: Some(spilled_head),
			},
			..*self
		}
	}

	/// The promote half of decompose-and-promote: derives the context for the
	/// content of one copy at a pushed structure level. `copy` becomes the
	/// enclosing level's index (held in `frame`, which the caller owns for the
	/// derived context's scope) and `inner` is the content's lane within that
	/// copy. The structure node computes `copy`/`inner` from its own
	/// extent-driven decomposition of the current lane.
	pub fn push_level<'s>(&self, frame: &'s mut IndexLink<'s>, copy: u64, inner: u64) -> ContextImpl<'s>
	where
		'a: 's,
	{
		frame.index = copy;
		frame.outer = self.index.outer;
		self.promoted(frame, inner)
	}
}

impl Ctx for ContextImpl<'_> {}

impl InjectIndex for ContextImpl<'_> {
	fn set_index(&mut self, index: u64) {
		self.index.index = index;
	}
}

impl ExtractFootprint for ContextImpl<'_> {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.footprint
	}
}
impl ExtractRealTime for ContextImpl<'_> {
	fn try_real_time(&self) -> Option<f64> {
		self.scope.real_time
	}
}
impl ExtractAnimationTime for ContextImpl<'_> {
	fn try_animation_time(&self) -> Option<f64> {
		self.scope.animation_time
	}
}
impl ExtractPointerPosition for ContextImpl<'_> {
	fn try_pointer_position(&self) -> Option<DVec2> {
		self.scope.pointer_position
	}
}
impl ExtractIndex for ContextImpl<'_> {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>> {
		Some(std::iter::successors(Some(&self.index), |link| link.outer).map(|link| link.index as usize))
	}
}
impl ExtractPosition for ContextImpl<'_> {
	fn try_position(&self) -> Option<impl Iterator<Item = DVec2>> {
		self.position.map(|head| std::iter::successors(Some(head), |link| link.outer).map(|link| link.position))
	}
}
impl ExtractVarArgs for ContextImpl<'_> {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		let mut link = self.varargs.ok_or(VarArgsResult::NoVarArgs)?;
		let mut remaining = index;
		loop {
			match link.args.get(remaining) {
				Some(arg) => return Ok(arg as DynRef<'_>),
				None => {
					remaining -= link.args.len();
					link = link.outer.ok_or(VarArgsResult::IndexOutOfBounds)?;
				}
			}
		}
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		let head = self.varargs.ok_or(VarArgsResult::NoVarArgs)?;
		Ok(std::iter::successors(Some(head), |link| link.outer).map(|link| link.args.len()).sum())
	}

	fn hash_varargs(&self, hasher: &mut dyn Hasher) {
		let mut count = 0u64;
		let mut link = self.varargs;
		while let Some(current) = link {
			for arg in current.args.iter() {
				arg.dyn_hash(&mut *hasher);
				count += 1;
			}
			link = current.outer;
		}
		count.hash(&mut &mut *hasher);
	}
}
impl<'a> ExtractArena for ContextImpl<'a> {
	type ArenaRef = &'a Arena;
	fn arena(&self) -> &'a Arena {
		self.scope.arena
	}
}

impl<'a> DeriveCtx for ContextImpl<'a> {
	type Family = ContextImplFamily;

	fn derived(&self) -> ContextImpl<'_> {
		*self
	}

	fn index_head(&self) -> IndexLink<'_> {
		self.index
	}

	fn scope(&self) -> &EvalScope<'_> {
		self.scope
	}

	fn position_head(&self) -> Option<&PositionLink<'_>> {
		self.position
	}

	fn varargs_head(&self) -> Option<&VarArgLink<'_>> {
		self.varargs
	}

	fn promoted<'s>(&'s self, spilled_head: &'s IndexLink<'s>, inner_index: u64) -> ContextImpl<'s> {
		ContextImpl::promoted(self, spilled_head, inner_index)
	}

	fn with_footprint<'s>(&'s self, footprint: &'s Footprint) -> ContextImpl<'s> {
		ContextImpl::with_footprint(self, footprint)
	}

	fn with_varargs<'s>(&'s self, varargs: &'s VarArgLink<'s>) -> ContextImpl<'s> {
		ContextImpl::with_varargs(self, varargs)
	}

	fn with_position<'s>(&'s self, position: &'s PositionLink<'s>) -> ContextImpl<'s> {
		ContextImpl::with_position(self, position)
	}

	fn with_scope<'s>(&'s self, scope: &'s EvalScope<'s>) -> ContextImpl<'s> {
		ContextImpl::with_scope(self, scope)
	}

	fn nullified<'s>(&'s self, keep: ContextFeatures, scope: &'s EvalScope<'s>) -> ContextImpl<'s> {
		ContextImpl::nullified(self, keep, scope)
	}
}

impl graphene_hash::CacheHash for ContextImpl<'_> {
	fn cache_hash<H: Hasher>(&self, state: &mut H) {
		match self.footprint {
			Some(footprint) => {
				1u8.hash(state);
				footprint.cache_hash(state);
			}
			None => 0u8.hash(state),
		}
		let mut count = 0u64;
		for link in std::iter::successors(Some(&self.index), |link| link.outer) {
			link.index.hash(state);
			count += 1;
		}
		count.hash(state);
		count = 0;
		let mut position = self.position;
		while let Some(link) = position {
			link.position.x.to_bits().hash(state);
			link.position.y.to_bits().hash(state);
			count += 1;
			position = link.outer;
		}
		count.hash(state);
		self.hash_varargs(state);
		self.scope.hash.hash(state);
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarArgsResult {
	IndexOutOfBounds,
	NoVarArgs,
}

#[cfg(test)]
mod context_impl_tests {
	use super::*;
	use crate::graphene_hash::CacheHash;

	fn hash_of(ctx: &ContextImpl) -> u64 {
		let mut hasher = std::hash::DefaultHasher::new();
		ctx.cache_hash(&mut hasher);
		hasher.finish()
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), Some(1.5), None, generations, arena)
	}

	#[test]
	fn equal_contexts_hash_equal() {
		let arena = Arena::new(64).unwrap();
		let generations = [(0, 1), (1, 3)];
		let scope = scope_fixture(&generations, &arena);
		let a = ContextImpl::root(&scope);
		let b = ContextImpl::root(&scope);
		assert_eq!(hash_of(&a), hash_of(&b));
	}

	#[test]
	fn each_axis_changes_the_hash() {
		let arena = Arena::new(64).unwrap();
		let generations = [(0, 1)];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);
		let footprint = Footprint::DEFAULT;

		let mut indexed = root;
		indexed.set_index(4);
		let position_link = PositionLink {
			position: DVec2::new(1.0, 2.0),
			outer: None,
		};
		let time_scope = scope.with_real_time(Some(9.75));
		let variants = [root.with_footprint(&footprint), indexed, root.with_position(&position_link), root.with_scope(&time_scope)];
		let root_hash = hash_of(&root);
		let mut hashes: Vec<u64> = variants.iter().map(hash_of).collect();
		hashes.push(root_hash);
		hashes.sort();
		hashes.dedup();
		assert_eq!(hashes.len(), variants.len() + 1, "every axis must contribute to the hash");
	}

	#[test]
	fn index_level_order_matters() {
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);

		let outer_one = IndexLink { index: 1, outer: None };
		let mut one_two = root.promoted(&outer_one, 2);
		let outer_two = IndexLink { index: 2, outer: None };
		let mut two_one = root.promoted(&outer_two, 1);
		assert_ne!(hash_of(&one_two), hash_of(&two_one));

		one_two.set_index(7);
		two_one.set_index(7);
		assert_ne!(hash_of(&one_two), hash_of(&two_one), "outer levels must stay hashed after set_index");
	}

	#[test]
	fn axis_boundaries_are_unambiguous() {
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);

		let position = PositionLink {
			position: DVec2::new(5.0, 5.0),
			outer: None,
		};
		let spilled = root.index_head();
		let mut deep_index = root.promoted(&spilled, 0);
		deep_index.set_index(5);
		let shallow_with_position = root.with_position(&position);
		assert_ne!(hash_of(&deep_index), hash_of(&shallow_with_position), "index levels must not blur into position levels");
	}

	#[test]
	fn retain_scopes_generation_invalidation() {
		let arena = Arena::new(64).unwrap();
		let initial = [(0, 1), (1, 3)];
		let bumped_unretained = [(0, 2), (1, 3)];
		let bumped_retained = [(0, 1), (1, 4)];

		let hash_with = |generations: &[(SourceId, u64)]| {
			let scope = scope_fixture(generations, &arena).nullified(ContextFeatures::all(), Some(&[1]));
			let retained_scope_context = ContextImpl::root(&scope);
			hash_of(&retained_scope_context)
		};
		assert_eq!(hash_with(&initial), hash_with(&bumped_unretained), "unretained source bumps must not invalidate");
		assert_ne!(hash_with(&initial), hash_with(&bumped_retained), "retained source bumps must invalidate");
	}

	#[test]
	fn excluding_keys_ignore_own_source_bumps() {
		let arena = Arena::new(64).unwrap();
		let initial = [(7, 1), (9, 5)];
		let own_bumped = [(7, 2), (9, 5)];
		let other_bumped = [(7, 1), (9, 6)];

		let hash_with = |generations: &[(SourceId, u64)]| {
			let scope = scope_fixture(generations, &arena).excluding(7);
			hash_of(&ContextImpl::root(&scope))
		};
		assert_eq!(hash_with(&initial), hash_with(&own_bumped), "a source's own generation bump must not change its slot key");
		assert_ne!(hash_with(&initial), hash_with(&other_bumped), "bumps of other sources must change the slot key");
	}

	#[test]
	fn unretained_scope_sees_every_bump() {
		let arena = Arena::new(64).unwrap();
		let initial = [(0, 1)];
		let bumped = [(0, 2)];
		let hash_with = |generations: &[(SourceId, u64)]| {
			let scope = scope_fixture(generations, &arena);
			hash_of(&ContextImpl::root(&scope))
		};
		assert_ne!(hash_with(&initial), hash_with(&bumped));
	}

	#[test]
	fn set_index_is_visible_and_keeps_outer_levels() {
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);
		let spilled = root.index_head();
		let mut ctx = root.promoted(&spilled, 0);
		ctx.set_index(11);
		let levels: Vec<usize> = ctx.try_index().unwrap().collect();
		assert_eq!(levels, vec![11, 0]);
	}

	#[test]
	fn vararg_chain_concatenates_innermost_first() {
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);

		let outer_value = 7u32;
		let outer_args: [DynSlot; 1] = [&outer_value];
		let outer_link = VarArgLink {
			args: VarArgSlots::Slice(&outer_args),
			outer: None,
		};
		let outer_ctx = root.with_varargs(&outer_link);

		let inner_value = String::from("inner");
		let inner_link = VarArgLink {
			args: VarArgSlots::Single(&inner_value),
			outer: Some(&outer_link),
		};
		let inner_ctx = root.with_varargs(&inner_link);

		assert_eq!(root.varargs_len(), Err(VarArgsResult::NoVarArgs));
		assert_eq!(outer_ctx.varargs_len(), Ok(1));
		assert_eq!(inner_ctx.varargs_len(), Ok(2));
		assert!(inner_ctx.vararg(0).unwrap().downcast_ref::<String>().is_some());
		assert!(inner_ctx.vararg(1).unwrap().downcast_ref::<u32>().is_some());
		assert!(matches!(inner_ctx.vararg(2), Err(VarArgsResult::IndexOutOfBounds)));
		assert_ne!(hash_of(&outer_ctx), hash_of(&inner_ctx));
	}

	#[test]
	fn snapshot_captures_vararg_levels() {
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);

		let outer_value = 7u32;
		let outer_args: [DynSlot; 1] = [&outer_value];
		let outer_link = VarArgLink {
			args: VarArgSlots::Slice(&outer_args),
			outer: None,
		};
		let inner_value = String::from("inner");
		let inner_link = VarArgLink {
			args: VarArgSlots::Single(&inner_value),
			outer: Some(&outer_link),
		};
		let ctx = root.with_varargs(&inner_link);

		let snapshot = CtxSnapshot::capture(&ctx);
		assert_eq!(snapshot.varargs_len(), Ok(2));
		assert_eq!(snapshot.vararg(0).unwrap().downcast_ref::<String>(), Some(&inner_value));
		assert_eq!(snapshot.vararg(1).unwrap().downcast_ref::<u32>(), Some(&outer_value));
		assert!(matches!(snapshot.vararg(2), Err(VarArgsResult::IndexOutOfBounds)));

		let hash_via = |target: &dyn Fn(&mut dyn Hasher)| {
			let mut hasher = std::hash::DefaultHasher::new();
			target(&mut hasher);
			hasher.finish()
		};
		assert_eq!(
			hash_via(&|hasher| snapshot.hash_varargs(hasher)),
			hash_via(&|hasher| ctx.hash_varargs(hasher)),
			"snapshot varargs must hash like the borrowed chain"
		);

		let cloned = snapshot.clone();
		assert_eq!(cloned.vararg(0).unwrap().downcast_ref::<String>(), Some(&inner_value));

		let empty = CtxSnapshot::capture(&root);
		assert_eq!(empty.varargs_len(), Err(VarArgsResult::NoVarArgs));
		assert!(matches!(empty.vararg(0), Err(VarArgsResult::NoVarArgs)));
	}

	#[test]
	fn sources_are_normalized_regardless_of_insertion_order() {
		let mut dependencies = ContextDependencies::default();
		dependencies.add_sources(&[9, 3, 9]);
		dependencies.add_sources(&[5, 1]);
		assert_eq!(dependencies.sources(), &[1, 3, 5, 9]);

		let modification = ContextModification::from_sources(ContextFeatures::empty(), &[7, 2, 7]);
		assert_eq!(modification.sources(), &[2, 7]);
	}

	#[test]
	fn a_snapshot_preserves_an_absent_position_axis() {
		let arena = Arena::new(64).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);

		let absent = CtxSnapshot::capture(&root);
		assert!(absent.try_position().is_none(), "capturing an unpositioned context must not invent a position stack");

		let position = DVec2::new(1., 2.);
		let positioned = CtxSnapshot::capture(&root.with_position(&PositionLink { position, outer: None }));
		assert_eq!(positioned.try_position().map(|p| p.collect::<Vec<_>>()), Some(vec![position]));
	}

	#[test]
	fn scope_arena_reaches_kernels_through_extract_arena() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let (value, _) = ExtractArena::arena(&ctx).alloc(41u32).unwrap();
		assert_eq!(*value, 41);
	}
}
