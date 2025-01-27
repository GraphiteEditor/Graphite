use core::{
	any::Any,
	borrow::Borrow,
	ops::{Deref, Index},
};
use std::sync::Arc;

use crate::transform::Footprint;
use dyn_any::DynAny;

pub trait Ctx: Clone + Send {}

pub trait ExtractFootprint {
	fn footprint(&self) -> Option<&Footprint>;
}

pub trait ExtractTime {
	fn time(&self) -> Option<f64>;
}

pub trait ExtractIndex {
	fn index(&self) -> Option<usize>;
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
	fn box_clone(&self) -> Box<dyn ExtractVarArgs + Send + Sync>;
}

pub trait ExtractAll: ExtractFootprint + ExtractIndex + ExtractTime + ExtractVarArgs {}

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
	fn footprint(&self) -> Option<&Footprint> {
		None
	}
}

impl<'n, T: ExtractFootprint + Ctx + Sync + Send> ExtractFootprint for &'n T {
	fn footprint(&self) -> Option<&Footprint> {
		(*self).footprint()
	}
}
impl<'n, T: ExtractFootprint> ExtractFootprint for Arc<T>
where
	Arc<T>: Ctx,
{
	fn footprint(&self) -> Option<&Footprint> {
		(**self).footprint()
	}
}

impl<T: ExtractFootprint + Sync> ExtractFootprint for Option<T> {
	fn footprint(&self) -> Option<&Footprint> {
		self.as_ref().and_then(|x| x.footprint())
	}
}
impl<T: ExtractTime + Sync> ExtractTime for Option<T> {
	fn time(&self) -> Option<f64> {
		self.as_ref().and_then(|x| x.time())
	}
}
impl<T: ExtractIndex> ExtractIndex for Option<T> {
	fn index(&self) -> Option<usize> {
		self.as_ref().and_then(|x| x.index())
	}
}
impl<T: ExtractVarArgs + Sync> ExtractVarArgs for Option<T> {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		let Some(ref inner) = self else { return Err(VarArgsResult::NoVarArgs) };
		inner.vararg(index)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		let Some(ref inner) = self else { return Err(VarArgsResult::NoVarArgs) };
		inner.varargs_len()
	}
}

impl<'a, T: ExtractVarArgs + Sync> ExtractVarArgs for &'a T {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		(*self).vararg(index)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		(*self).varargs_len()
	}
}

impl Ctx for ContextImpl<'_> {}
impl Ctx for Arc<OwnedContextImpl> {}

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
impl<'a> ExtractVarArgs for ContextImpl<'a> {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		let Some(inner) = self.varargs else { return Err(VarArgsResult::NoVarArgs) };
		inner.get(index).ok_or(VarArgsResult::IndexOutOfBounds).copied()
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		let Some(inner) = self.varargs else { return Err(VarArgsResult::NoVarArgs) };
		Ok(inner.len())
	}
}

impl ExtractFootprint for Arc<OwnedContextImpl> {
	fn footprint(&self) -> Option<&Footprint> {
		self.footprint.as_ref()
	}
}
impl ExtractTime for Arc<OwnedContextImpl> {
	fn time(&self) -> Option<f64> {
		self.time
	}
}
impl ExtractIndex for Arc<OwnedContextImpl> {
	fn index(&self) -> Option<usize> {
		self.index
	}
}
impl ExtractVarArgs for Arc<OwnedContextImpl> {
	fn vararg(&self, index: usize) -> Result<DynRef<'_>, VarArgsResult> {
		let Some(ref inner) = self.varargs else { return Err(VarArgsResult::NoVarArgs) };
		inner.get(index).map(|x| x.as_ref()).ok_or(VarArgsResult::IndexOutOfBounds)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		let Some(ref inner) = self.varargs else { return Err(VarArgsResult::NoVarArgs) };
		Ok(inner.len())
	}
}

pub type Context<'a> = Option<Arc<OwnedContextImpl>>;
type DynRef<'a> = &'a (dyn Any + Send + Sync);
type DynBox = Box<dyn Any + Send + Sync>;

#[derive(Default, dyn_any::DynAny)]
pub struct OwnedContextImpl {
	pub footprint: Option<crate::transform::Footprint>,
	pub varargs: Option<Arc<[DynBox]>>,
	pub parent: Option<Arc<dyn ExtractVarArgs + Sync + Send>>,
	// This could be converted into a single enum to save extra bytes
	pub index: Option<usize>,
	pub time: Option<f64>,
}

impl<T: ExtractAll + CloneVarArgs> From<T> for OwnedContextImpl {
	fn from(value: T) -> Self {
		let footprint = value.footprint().copied();
		let index = value.index();
		let time = value.time();
		let parent = value.box_clone();
		OwnedContextImpl {
			footprint,
			varargs: None,
			parent: Some(parent.into()),
			index,
			time,
		}
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
	// #[cfg(feature = "alloc")]
	// pub fn reborrow_var_args_to_vec<'short>(&self) -> Option<alloc::boxed::Box<[DynRef<'short>]>>
	// where
	// 	'a: 'short,
	// {
	// 	self.varargs.map(|x| shorten_lifetime_to_vec(x).into())
	// }
	// pub fn reborrow_var_args_to_buffer<'short, const N: usize>(&self, buffer: &'short mut [DynRef<'short>; N]) -> Option<&'short [DynRef<'short>]>
	// where
	// 	'a: 'short,
	// {
	// 	self.varargs.map(|x| shorten_lifetime_to_buffer(x, buffer))
	// }
}

// fn shorten_lifetime_to_vec<'c, 'b: 'c>(input: &'b [DynRef<'b>]) -> Vec<DynRef<'c>> {
// 	input.iter().map(|&x| x.reborrow_ref()).collect()
// }
// fn shorten_lifetime_to_buffer<'c, 'b: 'c, const N: usize>(input: &'b [DynRef<'b>], buffer: &'c mut [DynRef<'c>; N]) -> &'c [DynRef<'c>] {
// 	let iter = input.iter().map(|&x| x.reborrow_ref()).zip(buffer.iter_mut());
// 	if input.len() > N {
// 		unreachable!("Insufficient buffer size for varargs");
// 	}
// 	for (data, buffer_slot) in iter {
// 		*buffer_slot = data.reborrow_ref();
// 	}
// 	&buffer[..input.len()]
// }

// #[test]
// fn shorten_lifetime_compile_test() {
// 	let context: ContextImpl<'static> = const {
// 		ContextImpl {
// 			footprint: None,
// 			varargs: None,
// 			index: None,
// 			time: None,
// 		}
// 	};
// 	let footprint = Footprint::default();
// 	let local_varargs = context.reborrow_var_args_to_vec();
// 	let out = context.with_footprint(&footprint, local_varargs.as_ref());
// 	assert!(out.footprint().is_some());
// 	let mut buffer: [_; 0] = [];
// 	let local_varargs_buf = context.reborrow_var_args_to_buffer(&mut buffer);
// 	let out = context.with_footprint(&footprint, local_varargs_buf.as_ref());
// 	assert!(out.footprint().is_some());
// }
