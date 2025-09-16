use crate::transform::Footprint;
pub use graphene_core_shaders::context::{ArcCtx, Ctx};
use std::any::Any;
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::panic::Location;
use std::sync::Arc;

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

pub trait ExtractIndex {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>>;
}

// Consider returning a slice or something like that
pub trait ExtractVarArgs {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult>;
	fn varargs_len(&self) -> Result<usize, VarArgsResult>;
	fn hash_varargs(&self, hasher: &mut dyn Hasher);
}

// Consider returning a slice or something like that
pub trait CloneVarArgs: ExtractVarArgs {
	// fn box_clone(&self) -> Vec<DynBox>;
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>>;
}

// Inject* traits for providing context features to downstream nodes
pub trait InjectFootprint {}
pub trait InjectRealTime {}
pub trait InjectAnimationTime {}
pub trait InjectIndex {}
pub trait InjectVarArgs {}

// Modify* marker traits for context-transparent nodes
pub trait ModifyFootprint: ExtractFootprint + InjectFootprint {}
pub trait ModifyRealTime: ExtractRealTime + InjectRealTime {}
pub trait ModifyAnimationTime: ExtractAnimationTime + InjectAnimationTime {}
pub trait ModifyIndex: ExtractIndex + InjectIndex {}
pub trait ModifyVarArgs: ExtractVarArgs + InjectVarArgs {}

pub trait ExtractAll: ExtractFootprint + ExtractIndex + ExtractRealTime + ExtractAnimationTime + ExtractVarArgs {}

impl<T: ?Sized + ExtractFootprint + ExtractIndex + ExtractRealTime + ExtractAnimationTime + ExtractVarArgs> ExtractAll for T {}

impl<T: Ctx> InjectFootprint for T {}
impl<T: Ctx> InjectRealTime for T {}
impl<T: Ctx> InjectIndex for T {}
impl<T: Ctx> InjectAnimationTime for T {}
impl<T: Ctx> InjectVarArgs for T {}

impl<T: Ctx + InjectFootprint + ExtractFootprint> ModifyFootprint for T {}
impl<T: Ctx + InjectRealTime + ExtractRealTime> ModifyRealTime for T {}
impl<T: Ctx + InjectIndex + ExtractIndex> ModifyIndex for T {}
impl<T: Ctx + InjectAnimationTime + ExtractAnimationTime> ModifyAnimationTime for T {}
impl<T: Ctx + InjectVarArgs + ExtractVarArgs> ModifyVarArgs for T {}

// Public enum for flexible node macro codegen
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ContextFeature {
	ExtractFootprint,
	ExtractRealTime,
	ExtractAnimationTime,
	ExtractIndex,
	ExtractVarArgs,
	InjectFootprint,
	InjectRealTime,
	InjectAnimationTime,
	InjectIndex,
	InjectVarArgs,
}

// Internal bitflags for fast compiler analysis
use bitflags::bitflags;
bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, Default)]
	pub struct ContextFeatures: u32 {
		const FOOTPRINT = 1 << 0;
		const REAL_TIME = 1 << 1;
		const ANIMATION_TIME = 1 << 2;
		const INDEX = 1 << 3;
		const VARARGS = 1 << 4;
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, dyn_any::DynAny, serde::Serialize, serde::Deserialize, Default)]
pub struct ContextDependencies {
	pub extract: ContextFeatures,
	pub inject: ContextFeatures,
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
				ContextFeature::ExtractIndex => ContextFeatures::INDEX,
				ContextFeature::ExtractVarArgs => ContextFeatures::VARARGS,
				_ => ContextFeatures::empty(),
			};
			inject |= match feature {
				ContextFeature::InjectFootprint => ContextFeatures::FOOTPRINT,
				ContextFeature::InjectRealTime => ContextFeatures::REAL_TIME,
				ContextFeature::InjectAnimationTime => ContextFeatures::ANIMATION_TIME,
				ContextFeature::InjectIndex => ContextFeatures::INDEX,
				ContextFeature::InjectVarArgs => ContextFeatures::VARARGS,
				_ => ContextFeatures::empty(),
			};
		}
		Self { extract, inject }
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarArgsResult {
	IndexOutOfBounds,
	NoVarArgs,
}
impl Ctx for Footprint {}
impl ExtractFootprint for () {
	fn try_footprint(&self) -> Option<&Footprint> {
		log::error!("tried to extract footprint form (), {}", Location::caller());
		None
	}
}

impl<T: ExtractFootprint + Ctx + Sync + Send> ExtractFootprint for &T {
	fn try_footprint(&self) -> Option<&Footprint> {
		(*self).try_footprint()
	}
}

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
impl<T: CloneVarArgs + Sync> CloneVarArgs for Option<T> {
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>> {
		self.as_ref().and_then(CloneVarArgs::arc_clone)
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
impl<T: CloneVarArgs + Sync> CloneVarArgs for Arc<T> {
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>> {
		(**self).arc_clone()
	}
}

impl Ctx for ContextImpl<'_> {}
impl ArcCtx for OwnedContextImpl {}

impl ExtractFootprint for ContextImpl<'_> {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.footprint
	}
}
impl ExtractRealTime for ContextImpl<'_> {
	fn try_real_time(&self) -> Option<f64> {
		self.real_time
	}
}
impl ExtractIndex for ContextImpl<'_> {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>> {
		self.index.clone().map(|x| x.into_iter())
	}
}
impl ExtractVarArgs for ContextImpl<'_> {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		let Some(inner) = self.varargs else { return Err(VarArgsResult::NoVarArgs) };
		inner.get(index).ok_or(VarArgsResult::IndexOutOfBounds).copied()
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		let Some(inner) = self.varargs else { return Err(VarArgsResult::NoVarArgs) };
		Ok(inner.len())
	}

	fn hash_varargs(&self, _hasher: &mut dyn Hasher) {
		todo!()
	}
}

impl ExtractFootprint for OwnedContextImpl {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.footprint.as_ref()
	}
}
impl ExtractRealTime for OwnedContextImpl {
	fn try_real_time(&self) -> Option<f64> {
		self.real_time
	}
}
impl ExtractAnimationTime for OwnedContextImpl {
	fn try_animation_time(&self) -> Option<f64> {
		self.animation_time
	}
}
impl ExtractIndex for OwnedContextImpl {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>> {
		self.index.clone().map(|x| x.into_iter())
	}
}
impl ExtractVarArgs for OwnedContextImpl {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		let Some(ref inner) = self.varargs else {
			let Some(ref parent) = self.parent else {
				return Err(VarArgsResult::NoVarArgs);
			};
			return parent.vararg(index);
		};
		inner.get(index).map(|x| x.as_ref() as DynRef<'_>).ok_or(VarArgsResult::IndexOutOfBounds)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		let Some(ref inner) = self.varargs else {
			let Some(ref parent) = self.parent else {
				return Err(VarArgsResult::NoVarArgs);
			};
			return parent.varargs_len();
		};
		Ok(inner.len())
	}

	fn hash_varargs(&self, mut hasher: &mut dyn Hasher) {
		match (&self.varargs, &self.parent) {
			(Some(inner), _) => {
				for arg in inner.iter() {
					arg.hash(&mut hasher);
				}
			}
			(None, Some(parent)) => {
				parent.hash_varargs(hasher);
			}
			_ => (),
		};
	}
}

impl CloneVarArgs for Arc<OwnedContextImpl> {
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>> {
		Some(self.clone())
	}
}

pub type Context<'a> = Option<Arc<OwnedContextImpl>>;
type DynRef<'a> = &'a (dyn Any + Send + Sync);
type DynBox = Box<dyn AnyHash + Send + Sync>;

#[derive(dyn_any::DynAny)]
pub struct OwnedContextImpl {
	footprint: Option<Footprint>,
	varargs: Option<Arc<[DynBox]>>,
	parent: Option<Arc<dyn ExtractVarArgs + Sync + Send>>,
	// This could be converted into a single enum to save extra bytes
	index: Option<Vec<usize>>,
	real_time: Option<f64>,
	animation_time: Option<f64>,
}

impl std::fmt::Debug for OwnedContextImpl {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("OwnedContextImpl")
			.field("footprint", &self.footprint)
			.field("varargs_len", &self.varargs.as_ref().map(|x| x.len()))
			.field("parent", &self.parent.as_ref().map(|_| "<Parent>"))
			.field("index", &self.index)
			.field("real_time", &self.real_time)
			.field("animation_time", &self.animation_time)
			.finish()
	}
}

impl Default for OwnedContextImpl {
	#[track_caller]
	fn default() -> Self {
		Self::empty()
	}
}

impl Hash for OwnedContextImpl {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.footprint.hash(state);
		self.hash_varargs(state);
		self.index.hash(state);
		self.real_time.map(|x| x.to_bits()).hash(state);
		self.animation_time.map(|x| x.to_bits()).hash(state);
	}
}

impl OwnedContextImpl {
	#[track_caller]
	pub fn from<T: ExtractAll + CloneVarArgs>(value: T) -> Self {
		OwnedContextImpl::from_flags(value, ContextFeatures::all())
	}
	#[track_caller]
	pub fn from_flags<T: ExtractAll + CloneVarArgs>(value: T, bitflags: ContextFeatures) -> Self {
		let footprint = bitflags.contains(ContextFeatures::FOOTPRINT).then(|| value.try_footprint().copied()).flatten();
		let index = bitflags.contains(ContextFeatures::INDEX).then(|| value.try_index()).flatten();
		let real_time = bitflags.contains(ContextFeatures::REAL_TIME).then(|| value.try_real_time()).flatten();
		let animation_time = bitflags.contains(ContextFeatures::ANIMATION_TIME).then(|| value.try_animation_time()).flatten();
		let parent = bitflags
			.contains(ContextFeatures::VARARGS)
			.then(|| match value.varargs_len() {
				Ok(x) if x > 0 => value.arc_clone(),
				_ => None,
			})
			.flatten();

		OwnedContextImpl {
			footprint,
			varargs: None,
			parent,
			index: index.map(|x| x.collect()),
			real_time,
			animation_time,
		}
	}
	pub const fn empty() -> Self {
		OwnedContextImpl {
			footprint: None,
			varargs: None,
			parent: None,
			index: None,
			real_time: None,
			animation_time: None,
		}
	}
}

pub trait DynHash {
	fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<H: Hash + ?Sized> DynHash for H {
	fn dyn_hash(&self, mut state: &mut dyn Hasher) {
		self.hash(&mut state);
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

impl OwnedContextImpl {
	pub fn set_footprint(&mut self, footprint: Footprint) {
		self.footprint = Some(footprint);
	}
	pub fn with_footprint(mut self, footprint: Footprint) -> Self {
		self.footprint = Some(footprint);
		self
	}
	pub fn with_real_time(mut self, real_time: f64) -> Self {
		self.real_time = Some(real_time);
		self
	}
	pub fn with_animation_time(mut self, animation_time: f64) -> Self {
		self.animation_time = Some(animation_time);
		self
	}
	pub fn with_vararg(mut self, value: Box<dyn AnyHash + Send + Sync>) -> Self {
		assert!(self.varargs.is_none_or(|value| value.is_empty()));
		self.varargs = Some(Arc::new([value]));
		self
	}
	pub fn with_index(mut self, index: usize) -> Self {
		if let Some(current_index) = &mut self.index {
			current_index.push(index);
		} else {
			self.index = Some(vec![index]);
		}
		self
	}
	pub fn into_context(self) -> Option<Arc<Self>> {
		Some(Arc::new(self))
	}
	pub fn erase_parent(mut self) -> Self {
		self.parent = None;
		self
	}
}

#[derive(Default, Clone, dyn_any::DynAny)]
pub struct ContextImpl<'a> {
	pub(crate) footprint: Option<&'a Footprint>,
	varargs: Option<&'a [DynRef<'a>]>,
	index: Option<Vec<usize>>, // This could be converted into a single enum to save extra bytes
	real_time: Option<f64>,
}

impl<'a> ContextImpl<'a> {
	pub fn with_footprint<'f>(&self, new_footprint: &'f Footprint, varargs: Option<&'f impl Borrow<[DynRef<'f>]>>) -> ContextImpl<'f>
	where
		'a: 'f,
	{
		ContextImpl {
			footprint: Some(new_footprint),
			varargs: varargs.map(|x| x.borrow()),
			index: self.index.clone(),
			..*self
		}
	}
}
