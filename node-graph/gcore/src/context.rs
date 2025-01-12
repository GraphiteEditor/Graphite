use core::borrow::Borrow;

use crate::transform::Footprint;
use dyn_any::DynAny;

pub trait Ctx {}

pub trait ExtractFootprint: Ctx {
	fn footprint(&self) -> Option<&Footprint>;
}

pub trait ExtractTime: Ctx {
	fn time(&self) -> Option<f64>;
}

pub trait ExtractIndex: Ctx {
	fn index(&self) -> Option<usize>;
}

pub trait ExtractVarArgs: Ctx {
	// Call this lifetime 'b so it is less likely to coflict when auto generating the function signature for implementation
	fn vararg<'b>(&'b self, index: usize) -> Result<impl DynAny<'b> + Send + Sync, VarArgsResult>
	where
		Self: 'b;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarArgsResult {
	IndexOutOfBounds,
	NoVarArgs,
}
impl<T: Ctx> Ctx for Option<&T> {}

impl<T: ExtractFootprint> ExtractFootprint for Option<&T> {
	fn footprint(&self) -> Option<&Footprint> {
		self.and_then(|x| x.footprint())
	}
}
impl<T: ExtractTime> ExtractTime for Option<&T> {
	fn time(&self) -> Option<f64> {
		self.and_then(|x| x.time())
	}
}
impl<T: ExtractIndex> ExtractIndex for Option<&T> {
	fn index(&self) -> Option<usize> {
		self.and_then(|x| x.index())
	}
}
impl<T: ExtractVarArgs> ExtractVarArgs for Option<&T> {
	fn vararg<'b>(&'b self, index: usize) -> Result<impl DynAny<'b>, VarArgsResult>
	where
		Self: 'b,
	{
		let Some(inner) = self else { return Err(VarArgsResult::NoVarArgs) };
		inner.vararg(index)
	}
}

impl Ctx for ContextImpl<'_> {}

impl ExtractFootprint for ContextImpl<'_> {
	fn footprint(&self) -> Option<&Footprint> {
		self.footprint
	}
}
impl ExtractTime for ContextImpl<'_> {
	fn time(&self) -> Option<f64> {
		self.time
	}
}
impl ExtractIndex for ContextImpl<'_> {
	fn index(&self) -> Option<usize> {
		self.index
	}
}
impl ExtractVarArgs for ContextImpl<'_> {
	fn vararg<'b>(&'b self, index: usize) -> Result<impl DynAny<'b> + Send + Sync, VarArgsResult>
	where
		Self: 'b,
	{
		let Some(inner) = self.varargs else { return Err(VarArgsResult::NoVarArgs) };
		inner.get(index).ok_or(VarArgsResult::IndexOutOfBounds)
	}
}

pub type Context<'a> = Option<&'a ContextImpl<'a>>;
type DynRef<'a> = &'a (dyn DynAny<'a> + 'a + Send + Sync);

#[derive(Default, Clone, Copy, dyn_any::DynAny)]
pub struct ContextImpl<'a> {
	footprint: Option<&'a crate::transform::Footprint>,
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
	#[cfg(feature = "alloc")]
	pub fn reborrow_var_args_to_vec<'short>(&self) -> Option<alloc::boxed::Box<[DynRef<'short>]>>
	where
		'a: 'short,
	{
		self.varargs.map(|x| shorten_lifetime_to_vec(x).into())
	}
	pub fn reborrow_var_args_to_buffer<'short, const N: usize>(&self, buffer: &'short mut [DynRef<'short>; N]) -> Option<&'short [DynRef<'short>]>
	where
		'a: 'short,
	{
		self.varargs.map(|x| shorten_lifetime_to_buffer(x, buffer))
	}
}

fn shorten_lifetime_to_vec<'c, 'b: 'c>(input: &'b [DynRef<'b>]) -> Vec<DynRef<'c>> {
	input.iter().map(|&x| x.reborrow_ref()).collect()
}
fn shorten_lifetime_to_buffer<'c, 'b: 'c, const N: usize>(input: &'b [DynRef<'b>], buffer: &'c mut [DynRef<'c>; N]) -> &'c [DynRef<'c>] {
	let iter = input.iter().map(|&x| x.reborrow_ref()).zip(buffer.iter_mut());
	if input.len() > N {
		unreachable!("Insufficient buffer size for varargs");
	}
	for (data, buffer_slot) in iter {
		*buffer_slot = data.reborrow_ref();
	}
	&buffer[..input.len()]
}

#[test]
fn shorten_lifetime_compile_test() {
	let context: ContextImpl<'static> = const {
		ContextImpl {
			footprint: None,
			varargs: None,
			index: None,
			time: None,
		}
	};
	let footprint = Footprint::default();
	let local_varargs = context.reborrow_var_args_to_vec();
	let out = context.with_footprint(&footprint, local_varargs.as_ref());
	assert!(out.footprint().is_some());
	let mut buffer: [_; 0] = [];
	let local_varargs_buf = context.reborrow_var_args_to_buffer(&mut buffer);
	let out = context.with_footprint(&footprint, local_varargs_buf.as_ref());
	assert!(out.footprint().is_some());
}
