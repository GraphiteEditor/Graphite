use crate::uuid::NodeId;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
pub type Task = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
#[cfg(target_family = "wasm")]
pub type Task = Pin<Box<dyn Future<Output = ()> + 'static>>;

pub trait Spawn {
	fn spawn(&self, task: Task);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, graphene_hash::CacheHash, dyn_any::DynAny)]
pub struct SourceId(pub u64);

impl SourceId {
	pub fn from_path(path: &[NodeId]) -> Self {
		let mut hasher = rustc_hash::FxHasher::default();
		path.hash(&mut hasher);
		Self(hasher.finish())
	}
}

pub type Generations = Arc<[(SourceId, u64)]>;
