use dyn_any::DynAny;
use std::any::Any;
use std::sync::{Arc, OnceLock};

use crate::WgpuExecutor;

pub trait Pipeline: Any + Send + Sync + Sized {
	type Args<'a>;
	type Out: Send;

	fn create(executor: &WgpuExecutor) -> Self;

	fn run(&self, executor: &WgpuExecutor, args: &Self::Args<'_>) -> Self::Out;
}

#[derive(Default, Clone, DynAny)]
pub struct PipelineCache {
	pipeline: Arc<OnceLock<Box<dyn Any + Send + Sync>>>,
	executor: Arc<OnceLock<WgpuExecutor>>,
}

impl PipelineCache {
	pub(super) fn init<P: Pipeline>(&self, executor: &WgpuExecutor) {
		self.executor.get_or_init(|| executor.clone());
		self.pipeline.get_or_init(|| Box::new(P::create(executor)));
	}

	pub fn run<P: Pipeline>(&self, args: &P::Args<'_>) -> Result<P::Out, PipelineError> {
		let (Some(executor), Some(entry)) = (self.executor.get(), self.pipeline.get()) else {
			return Err(PipelineError::NotInitialized);
		};
		let pipeline = (**entry).downcast_ref::<P>().ok_or(PipelineError::TypeMismatch { expected: std::any::type_name::<P>() })?;
		Ok(pipeline.run(executor, args))
	}

	/// The executor the cache was initialized with, or `None` when no GPU is available.
	pub fn executor(&self) -> Option<&WgpuExecutor> {
		self.executor.get()
	}

	/// Runs the pipeline, or returns `None` when it was never initialized because no GPU is available.
	pub fn try_run<P: Pipeline>(&self, args: &P::Args<'_>) -> Option<P::Out> {
		self.executor.get()?;
		self.run::<P>(args).ok()
	}
}

impl std::fmt::Debug for PipelineCache {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PipelineCache").field("initialized", &self.pipeline.get().is_some()).finish()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PipelineError {
	#[error("The GPU pipeline was not set up")]
	NotInitialized,
	#[error("The GPU pipeline cache holds a pipeline other than `{expected}`")]
	TypeMismatch { expected: &'static str },
}
