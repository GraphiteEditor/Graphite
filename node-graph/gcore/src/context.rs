use crate::transform::Footprint;
use core::any::Any;
use core::borrow::Borrow;
use core::panic::Location;
use std::sync::Arc;

pub trait Ctx: Clone + Send {}

pub trait ExtractFootprint {
	#[track_caller]
	fn try_footprint(&self) -> Option<&Footprint>;
	#[track_caller]
	fn footprint(&self) -> &Footprint {
		self.try_footprint().unwrap_or_else(|| {
			log::error!("Context did not have a footprint, called from: {}", Location::caller());
			&const { Footprint::empty() }
		})
	}
}

pub trait ExtractTime {
	fn try_time(&self) -> Option<f64>;
}

pub trait ExtractAnimationTime {
	fn try_animation_time(&self) -> Option<f64>;
}

pub trait ExtractIndex {
	fn try_index(&self) -> Option<usize>;
}

// Consider returning a slice or something like that
pub trait ExtractVarArgs {
	// Call this lifetime 'b so it is less likely to coflict when auto generating the function signature for implementation
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult>;
	fn varargs_len(&self) -> Result<usize, VarArgsResult>;
}
// Consider returning a slice or something like that
pub trait CloneVarArgs: ExtractVarArgs {
	// fn box_clone(&self) -> Vec<DynBox>;
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>>;
}

pub trait ExtractAll: ExtractFootprint + ExtractIndex + ExtractTime + ExtractAnimationTime + ExtractVarArgs {}

impl<T: ?Sized + ExtractFootprint + ExtractIndex + ExtractTime + ExtractAnimationTime + ExtractVarArgs> ExtractAll for T {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarArgsResult {
	IndexOutOfBounds,
	NoVarArgs,
}
impl<T: Ctx> Ctx for Option<T> {}
impl<T: Ctx + Sync> Ctx for &T {}
impl Ctx for () {}
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
			&const { Footprint::empty() }
		})
	}
}
impl<T: ExtractTime + Sync> ExtractTime for Option<T> {
	fn try_time(&self) -> Option<f64> {
		self.as_ref().and_then(|x| x.try_time())
	}
}
impl<T: ExtractAnimationTime + Sync> ExtractAnimationTime for Option<T> {
	fn try_animation_time(&self) -> Option<f64> {
		self.as_ref().and_then(|x| x.try_animation_time())
	}
}
impl<T: ExtractIndex> ExtractIndex for Option<T> {
	fn try_index(&self) -> Option<usize> {
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
}
impl<T: ExtractFootprint + Sync> ExtractFootprint for Arc<T> {
	fn try_footprint(&self) -> Option<&Footprint> {
		(**self).try_footprint()
	}
}
impl<T: ExtractTime + Sync> ExtractTime for Arc<T> {
	fn try_time(&self) -> Option<f64> {
		(**self).try_time()
	}
}
impl<T: ExtractAnimationTime + Sync> ExtractAnimationTime for Arc<T> {
	fn try_animation_time(&self) -> Option<f64> {
		(**self).try_animation_time()
	}
}
impl<T: ExtractIndex> ExtractIndex for Arc<T> {
	fn try_index(&self) -> Option<usize> {
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
}
impl<T: CloneVarArgs + Sync> CloneVarArgs for Arc<T> {
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>> {
		(**self).arc_clone()
	}
}

impl Ctx for ContextImpl<'_> {}
impl Ctx for Arc<OwnedContextImpl> {}

impl ExtractFootprint for ContextImpl<'_> {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.footprint
	}
}
impl ExtractTime for ContextImpl<'_> {
	fn try_time(&self) -> Option<f64> {
		self.time
	}
}
impl ExtractIndex for ContextImpl<'_> {
	fn try_index(&self) -> Option<usize> {
		self.index
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
}

impl ExtractFootprint for OwnedContextImpl {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.footprint.as_ref()
	}
}
impl ExtractTime for OwnedContextImpl {
	fn try_time(&self) -> Option<f64> {
		self.time
	}
}
impl ExtractAnimationTime for OwnedContextImpl {
	fn try_animation_time(&self) -> Option<f64> {
		self.animation_time
	}
}
impl ExtractIndex for OwnedContextImpl {
	fn try_index(&self) -> Option<usize> {
		self.index
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
		inner.get(index).map(|x| x.as_ref()).ok_or(VarArgsResult::IndexOutOfBounds)
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
}

impl CloneVarArgs for Arc<OwnedContextImpl> {
	fn arc_clone(&self) -> Option<Arc<dyn ExtractVarArgs + Send + Sync>> {
		Some(self.clone())
	}
}

pub type Context<'a> = Option<Arc<OwnedContextImpl>>;
type DynRef<'a> = &'a (dyn Any + Send + Sync);
type DynBox = Box<dyn Any + Send + Sync>;

#[derive(dyn_any::DynAny)]
pub struct OwnedContextImpl {
	footprint: Option<crate::transform::Footprint>,
	varargs: Option<Arc<[DynBox]>>,
	parent: Option<Arc<dyn ExtractVarArgs + Sync + Send>>,
	// This could be converted into a single enum to save extra bytes
	index: Option<usize>,
	time: Option<f64>,
	animation_time: Option<f64>,
}

impl Default for OwnedContextImpl {
	#[track_caller]
	fn default() -> Self {
		Self::empty()
	}
}

impl core::hash::Hash for OwnedContextImpl {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.footprint.hash(state);
		self.index.hash(state);
		self.time.map(|x| x.to_bits()).hash(state);
		self.parent.as_ref().map(|x| Arc::as_ptr(x).addr()).hash(state);
		self.varargs.as_ref().map(|x| Arc::as_ptr(x).addr()).hash(state);
	}
}

impl OwnedContextImpl {
	#[track_caller]
	pub fn from<T: ExtractAll + CloneVarArgs>(value: T) -> Self {
		let footprint = value.try_footprint().copied();
		let index = value.try_index();
		let time = value.try_time();
		let frame_time = value.try_animation_time();
		let parent = value.arc_clone();
		OwnedContextImpl {
			footprint,
			varargs: None,
			parent,
			index,
			time,
			animation_time: frame_time,
		}
	}
	pub const fn empty() -> Self {
		OwnedContextImpl {
			footprint: None,
			varargs: None,
			parent: None,
			index: None,
			time: None,
			animation_time: None,
		}
	}
}

impl OwnedContextImpl {
	pub fn set_footprint(&mut self, footprint: Footprint) {
		self.footprint = Some(footprint);
	}
	pub fn with_footprint(mut self, footprint: Footprint) -> Self {
		self.footprint = Some(footprint);
		self
	}
	pub fn with_time(mut self, time: f64) -> Self {
		self.time = Some(time);
		self
	}
	pub fn with_animation_time(mut self, animation_time: f64) -> Self {
		self.animation_time = Some(animation_time);
		self
	}
	pub fn into_context(self) -> Option<Arc<Self>> {
		Some(Arc::new(self))
	}
}

#[derive(Default, Clone, Copy, dyn_any::DynAny)]
pub struct ContextImpl<'a> {
	pub(crate) footprint: Option<&'a crate::transform::Footprint>,
	varargs: Option<&'a [DynRef<'a>]>,
	// This could be converted into a single enum to save extra bytes
	index: Option<usize>,
	time: Option<f64>,
}

impl<'a> ContextImpl<'a> {
	pub fn with_footprint<'f>(&self, new_footprint: &'f Footprint, varargs: Option<&'f impl (Borrow<[DynRef<'f>]>)>) -> ContextImpl<'f>
	where
		'a: 'f,
	{
		ContextImpl {
			footprint: Some(new_footprint),
			varargs: varargs.map(|x| x.borrow()),
			..*self
		}
	}
}
