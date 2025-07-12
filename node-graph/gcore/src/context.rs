use glam::{DAffine2, UVec2};

use crate::transform::Footprint;
use std::any::Any;
use std::panic::Location;
use std::sync::Arc;

pub trait Ctx: Clone + Send {}

pub trait ExtractFootprint {
	fn try_footprint(&self) -> Option<&Footprint>;
	#[track_caller]
	fn footprint(&self) -> &Footprint {
		self.try_footprint().unwrap_or_else(|| {
			log::error!("Context did not have a footprint, called from: {}", Location::caller());
			&Footprint::DEFAULT
		})
	}
}

pub trait ExtractDownstreamTransform {
	fn try_downstream_transform(&self) -> Option<&DAffine2>;
}

pub trait ExtractRealTime {
	fn try_real_time(&self) -> Option<f64>;
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

pub trait ExtractAll: ExtractFootprint + ExtractDownstreamTransform + ExtractIndex + ExtractRealTime + ExtractAnimationTime + ExtractVarArgs {}

impl<T: ?Sized + ExtractFootprint + ExtractDownstreamTransform + ExtractIndex + ExtractRealTime + ExtractAnimationTime + ExtractVarArgs> ExtractAll for T {}

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum ContextDependency {
	ExtractFootprint = 0b10000000,
	// Can be used by cull nodes to check if the final output would be outside the footprint viewport
	ExtractDownstreamTransform = 0b01000000,
	ExtractRealTime = 0b00100000,
	ExtractAnimationTime = 0b00010000,
	ExtractIndex = 0b00001000,
	ExtractVarArgs = 0b00000100,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextDependencies(pub u8);

impl ContextDependencies {
	pub fn all_context_dependencies() -> Self {
		ContextDependencies(0b11111100)
	}

	pub fn none() -> Self {
		ContextDependencies(0b00000000)
	}

	pub fn is_empty(&self) -> bool {
		self.0 & Self::all_context_dependencies().0 == 0
	}

	pub fn from(dependencies: Vec<ContextDependency>) -> Self {
		let mut new = Self::none();
		for dependency in dependencies {
			new.0 |= dependency as u8
		}
		new
	}

	pub fn inverse(self) -> Self {
		Self(!self.0)
	}

	pub fn add_dependencies(&mut self, other: &Self) {
		self.0 |= other.0
	}

	pub fn difference(&mut self, other: &Self) {
		self.0 = (!self.0) & other.0
	}
}

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
}

impl ExtractDownstreamTransform for () {
	fn try_downstream_transform(&self) -> Option<&DAffine2> {
		log::error!("tried to extract downstream transform form (), {}", Location::caller());
		None
	}
}

impl<T: ExtractDownstreamTransform + Ctx + Sync + Send> ExtractDownstreamTransform for &T {
	fn try_downstream_transform(&self) -> Option<&DAffine2> {
		(*self).try_downstream_transform()
	}
}

impl<T: ExtractDownstreamTransform + Sync> ExtractDownstreamTransform for Option<T> {
	fn try_downstream_transform(&self) -> Option<&DAffine2> {
		self.as_ref().and_then(|x| x.try_downstream_transform())
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

impl<T: ExtractDownstreamTransform + Sync> ExtractDownstreamTransform for Arc<T> {
	fn try_downstream_transform(&self) -> Option<&DAffine2> {
		(**self).try_downstream_transform()
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

impl Ctx for Arc<OwnedContextImpl> {}

impl ExtractFootprint for OwnedContextImpl {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.footprint.as_ref()
	}
}

impl ExtractDownstreamTransform for OwnedContextImpl {
	fn try_downstream_transform(&self) -> Option<&DAffine2> {
		self.downstream_transform.as_ref()
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
	// The footprint represents the document to viewport render metadata
	footprint: Option<Footprint>,
	// The transform node does not modify the document to viewport, it instead modifies this,
	// which can be used to transform the evaluated data from the node and check if it is within the
	// document to viewport transform.
	downstream_transform: Option<DAffine2>,
	index: Option<usize>,
	real_time: Option<f64>,
	animation_time: Option<f64>,

	// varargs: Option<(Vec<String>, Arc<[DynBox]>)>,
	varargs: Option<Arc<[DynBox]>>,

	parent: Option<Arc<dyn ExtractVarArgs + Sync + Send>>,
}

impl std::fmt::Debug for OwnedContextImpl {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("OwnedContextImpl")
			.field("footprint", &self.footprint)
			.field("varargs", &self.varargs)
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

impl std::hash::Hash for OwnedContextImpl {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.footprint.hash(state);
		self.varargs.as_ref().map(|x| Arc::as_ptr(x).addr()).hash(state);
		self.parent.as_ref().map(|x| Arc::as_ptr(x).addr()).hash(state);
		self.index.hash(state);
		self.real_time.map(|x| x.to_bits()).hash(state);
		self.animation_time.map(|x| x.to_bits()).hash(state);
	}
}

impl OwnedContextImpl {
	#[track_caller]
	pub fn from<T: ExtractAll + CloneVarArgs>(value: T) -> Self {
		let footprint = value.try_footprint().copied();
		let downstream_transform = value.try_downstream_transform().copied();
		let index = value.try_index();
		let time = value.try_real_time();
		let frame_time = value.try_animation_time();
		let parent = match value.varargs_len() {
			Ok(x) if x > 0 => value.arc_clone(),
			_ => None,
		};
		OwnedContextImpl {
			footprint,
			downstream_transform,
			index,
			real_time: time,
			animation_time: frame_time,
			varargs: None,
			parent,
		}
	}

	pub const fn empty() -> Self {
		OwnedContextImpl {
			footprint: None,
			downstream_transform: None,
			index: None,
			real_time: None,
			animation_time: None,
			varargs: None,
			parent: None,
		}
	}

	pub fn nullify(&mut self, nullify: &ContextDependencies) {
		if nullify.0 & (ContextDependency::ExtractFootprint as u8) != 0 {
			self.footprint = None;
		}
		if nullify.0 & (ContextDependency::ExtractDownstreamTransform as u8) != 0 {
			self.downstream_transform = None;
		}
		if nullify.0 & (ContextDependency::ExtractRealTime as u8) != 0 {
			self.real_time = None;
		}
		if nullify.0 & (ContextDependency::ExtractAnimationTime as u8) != 0 {
			self.animation_time = None;
		}
		if nullify.0 & (ContextDependency::ExtractIndex as u8) != 0 {
			self.index = None;
		}
		if nullify.0 & (ContextDependency::ExtractVarArgs as u8) != 0 {
			self.varargs = None;
			self.parent = None
		}
	}
}

impl OwnedContextImpl {
	pub fn set_footprint(&mut self, footprint: Footprint) {
		self.footprint = Some(footprint);
	}
	pub fn set_downstream_transform(&mut self, transform: DAffine2) {
		self.downstream_transform = Some(transform);
	}
	pub fn try_apply_downstream_transform(&mut self, transform: DAffine2) {
		if let Some(downstream_transform) = self.downstream_transform {
			self.downstream_transform = Some(downstream_transform * transform);
		}
	}
	pub fn set_real_time(&mut self, time: f64) {
		self.real_time = Some(time);
	}
	pub fn set_animation_time(&mut self, animation_time: f64) {
		self.animation_time = Some(animation_time);
	}
	pub fn set_index(&mut self, index: usize) {
		self.index = Some(index);
	}
	pub fn with_footprint(mut self, footprint: Footprint) -> Self {
		self.footprint = Some(footprint);
		self
	}
	pub fn with_downstream_transform(mut self, downstream_transform: DAffine2) -> Self {
		self.downstream_transform = Some(downstream_transform);
		self
	}
	pub fn with_real_time(mut self, time: f64) -> Self {
		self.real_time = Some(time);
		self
	}
	pub fn with_animation_time(mut self, animation_time: f64) -> Self {
		self.animation_time = Some(animation_time);
		self
	}
	pub fn with_index(mut self, index: usize) -> Self {
		self.index = Some(index);
		self
	}
	pub fn into_context(self) -> Option<Arc<Self>> {
		Some(Arc::new(self))
	}
	pub fn add_vararg(mut self, _variable_name: String, value: Box<dyn Any + Send + Sync>) -> Self {
		assert!(self.varargs.is_none_or(|value| value.is_empty()));
		// self.varargs = Some((vec![variable_name], Arc::new([value])));
		self.varargs = Some(Arc::new([value]));

		self
	}
	pub fn set_varargs(&mut self, var_args: (Vec<String>, Arc<[DynBox]>)) {
		self.varargs = Some(var_args.1)
	}
	pub fn with_vararg(mut self, var_args: (impl Into<String>, DynBox)) -> Self {
		self.varargs = Some(Arc::new([var_args.1]));
		self
	}
	pub fn erase_parent(mut self) -> Self {
		self.parent = None;
		self
	}
}

// #[derive(Default, Clone, Copy, dyn_any::DynAny)]
// pub struct ContextImpl<'a> {
// 	pub(crate) footprint: Option<&'a Footprint>,
// 	varargs: Option<&'a [DynRef<'a>]>,
// 	// This could be converted into a single enum to save extra bytes
// 	index: Option<usize>,
// 	time: Option<f64>,
// }

// impl<'a> ContextImpl<'a> {
// 	pub fn with_footprint<'f>(&self, new_footprint: &'f Footprint, varargs: Option<&'f impl Borrow<[DynRef<'f>]>>) -> ContextImpl<'f>
// 	where
// 		'a: 'f,
// 	{
// 		ContextImpl {
// 			footprint: Some(new_footprint),
// 			varargs: varargs.map(|x| x.borrow()),
// 			..*self
// 		}
// 	}
// }

#[node_macro::node(category("Context Getter"))]
fn get_footprint(ctx: impl Ctx + ExtractFootprint) -> Option<Footprint> {
	ctx.try_footprint().copied()
}

#[node_macro::node(category("Context Getter"))]
fn get_document_to_viewport(ctx: impl Ctx + ExtractFootprint) -> Option<DAffine2> {
	ctx.try_footprint().map(|footprint| footprint.transform.clone())
}

#[node_macro::node(category("Context Getter"))]
fn get_resolution(ctx: impl Ctx + ExtractFootprint) -> Option<UVec2> {
	ctx.try_footprint().map(|footprint| footprint.resolution.clone())
}

#[node_macro::node(category("Context Getter"))]
fn get_downstream_transform(ctx: impl Ctx + ExtractDownstreamTransform) -> Option<DAffine2> {
	ctx.try_downstream_transform().copied()
}

#[node_macro::node(category("Context Getter"))]
fn get_real_time(ctx: impl Ctx + ExtractRealTime) -> Option<f64> {
	ctx.try_real_time()
}

#[node_macro::node(category("Context Getter"))]
fn get_animation_time(ctx: impl Ctx + ExtractAnimationTime) -> Option<f64> {
	ctx.try_animation_time()
}

#[node_macro::node(category("Context Getter"))]
fn get_index(ctx: impl Ctx + ExtractIndex) -> Option<u32> {
	ctx.try_index().map(|index| index as u32)
}

// #[node_macro::node(category("Loop"))]
// async fn loop_node<T: Default>(
// 	ctx: impl Ctx + CloneVarArgs + ExtractAll,
// 	#[implementations(
//         Context -> Option<u32>,
// 	)]
// 	return_if_some: impl Node<Context<'static>, Output = Option<T>>,
// 	#[implementations(
//         Context -> (),
// 	)]
// 	run_if_none: impl Node<Context<'static>, Output = ()>,
// ) -> T {
// 	let mut context = OwnedContextImpl::from(ctx.clone());
// 	context.arc_mutex = Some(Arc::new(Mutex::new(None)));
// 	loop {
// 		if let Some(return_value) = return_if_some.eval(context.clone().into_context()).await {
// 			return return_value;
// 		}
// 		run_if_none.eval(context.clone().into_context()).await;
// 		let Some(context_after_loop) = context.arc_mutex.unwrap().lock().unwrap().take() else {
// 			log::error!("Loop context was not set, breaking loop to avoid infinite loop");
// 			return T::default();
// 		};
// 		context = context_after_loop;
// 		context.arc_mutex = Some(Arc::new(Mutex::new(None)));
// 	}
// }

// #[node_macro::node(category("Loop"))]
// async fn update_loop_node_context(ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync) -> () {
// 	let mut context = OwnedContextImpl::from(ctx.clone());
// 	let context_after_loop = OwnedContextImpl::from(ctx.clone());
// 	if let Some(arc_mutex) = context.arc_mutex.as_ref() {
// 		*arc_mutex.lock().unwrap() = Some(context_after_loop);
// 	}
// }

#[node_macro::node(category("Loop"))]
async fn set_index<T: 'n + 'static>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync,
	#[expose]
	#[implementations(
        Context -> u32,
		Context -> (),
	)]
	input: impl Node<Context<'static>, Output = T>,
	number: u32,
) -> T {
	let mut new_context = OwnedContextImpl::from(ctx);
	new_context.index = Some(number.try_into().unwrap());
	input.eval(new_context.into_context()).await
}

// #[node_macro::node(category("Loop"))]
// fn create_arc_mutex(_ctx: impl Ctx) -> Arc<Mutex<Option<OwnedContextImpl>>> {
// 	Arc::new(Mutex::new(0))
// }

// #[node_macro::node(category("Loop"))]
// fn get_arc_mutex(ctx: impl Ctx + ExtractArcMutex) -> Option<Arc<Mutex<Option<OwnedContextImpl>>>> {
// 	ctx.try_arc_mutex()
// }

// #[node_macro::node(category("Loop"))]
// async fn set_arc_mutex<T: 'n + 'static>(
// 	ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync,
// 	// Auto generate for each tagged value type
// 	#[expose]
// 	#[implementations(
//         Context -> u32,
// 	)]
// 	input: impl Node<Context<'static>, Output = T>,
// 	arc_mutex: Arc<Mutex<Option<OwnedContextImpl>>>,
// ) -> T {
// 	let mut new_context = OwnedContextImpl::from(ctx);
// 	new_context.arc_mutex = Some(arc_mutex);
// 	input.eval(new_context.into_context()).await
// }

// // TODO: Discard node + return () for loop if none branch
// #[node_macro::node(category("Loop"))]
// fn set_arc_mutex_value<T>(_ctx: impl Ctx, #[implementations(Arc<Mutex<u32>>)] arc_mutex: Arc<Mutex<T>>, #[implementations(u32)] value: T) -> () {
// 	let mut guard = arc_mutex.lock().unwrap(); // lock the mutex
// 	*guard = value;
// }

// #[node_macro::node(category("Loop"))]
// fn get_context(ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync) -> OwnedContextImpl {
// 	OwnedContextImpl::from(ctx)
// }

// #[node_macro::node(category("Loop"))]
// fn discard(_ctx: impl Ctx, _input: u32) -> () {}

#[node_macro::node(category("Debug"))]
fn is_none<T>(_: impl Ctx, #[implementations(Option<f64>, Option<f32>, Option<u32>, Option<u64>, Option<String>)] input: Option<T>) -> bool {
	input.is_none()
}

// #[node_macro::node(category("Debug"))]
// fn unwrap<T>(_: impl Ctx, #[implementations(Option<f64>, Option<f32>, Option<u32>, Option<u64>, Option<String>, Option<Vec<u32>>)] input: Option<T>) -> T {
// 	input.unwrap()
// }

#[node_macro::node(category("Debug"))]
fn to_option<T>(_: impl Ctx, boolean: bool, #[implementations(u32)] input: T) -> Option<T> {
	boolean.then(|| input)
}

#[node_macro::node(category("Debug"))]
fn to_usize(_: impl Ctx, u32: u32) -> usize {
	u32.try_into().unwrap()
}
