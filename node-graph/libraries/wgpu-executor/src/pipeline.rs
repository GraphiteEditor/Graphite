use dyn_any::StaticType;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use crate::WgpuExecutor;

pub type PipelineFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Pipeline: Any + Send + Sync + Sized {
	type Args<'a>;
	type Out: Send;

	fn create(executor: &WgpuExecutor) -> Self;

	fn run<'a>(&'a self, executor: &'a WgpuExecutor, args: &'a Self::Args<'_>) -> PipelineFuture<'a, Self::Out>;
}

pub trait AsyncPipeline: Any + Send + Sync + Sized {
	type Args<'a>;
	type Out: Send;

	fn create(executor: &WgpuExecutor) -> Self;

	fn run<'a>(&'a self, executor: &'a WgpuExecutor, args: &'a Self::Args<'_>) -> impl Future<Output = Self::Out> + Send + 'a;
}

impl<P: AsyncPipeline> Pipeline for P {
	type Args<'a> = <P as AsyncPipeline>::Args<'a>;
	type Out = <P as AsyncPipeline>::Out;

	fn create(executor: &WgpuExecutor) -> Self {
		<P as AsyncPipeline>::create(executor)
	}

	fn run<'a>(&'a self, executor: &'a WgpuExecutor, args: &'a Self::Args<'_>) -> PipelineFuture<'a, Self::Out> {
		Box::pin(<P as AsyncPipeline>::run(self, executor, args))
	}
}

#[derive(Default, Clone)]
pub struct PipelineCache {
	pipeline: Arc<OnceLock<Box<dyn Any + Send + Sync>>>,
	executor: Arc<OnceLock<WgpuExecutor>>,
}

impl PipelineCache {
	pub(super) fn init<P: Pipeline>(&self, executor: &WgpuExecutor) {
		self.executor.get_or_init(|| executor.clone());
		self.pipeline.get_or_init(|| Box::new(P::create(executor)));
	}

	pub async fn run<P: Pipeline>(&self, args: &P::Args<'_>) -> P::Out {
		let executor = self.executor.get().expect("PipelineCache not initialized");
		let entry = self.pipeline.get().expect("PipelineCache not initialized");
		let pipeline = (&**entry)
			.downcast_ref::<P>()
			.unwrap_or_else(|| panic!("PipelineCache type mismatch: run::<{}>() but init used a different pipeline type", std::any::type_name::<P>(),));
		pipeline.run(executor, args).await
	}
}

impl std::fmt::Debug for PipelineCache {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PipelineCache").field("initialized", &self.pipeline.get().is_some()).finish()
	}
}

unsafe impl StaticType for PipelineCache {
	type Static = PipelineCache;
}
