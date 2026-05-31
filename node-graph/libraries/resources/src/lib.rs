use core_types::graphene_hash;
use core_types::resource::{Resource, ResourceHash};
use dyn_any::DynAny;
use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;

pub trait LoadResource: Send + Sync {
	fn load(&self, hash: ResourceHash) -> ResourceFuture;
}

pub type ResourceFuture = Pin<Box<dyn Future<Output = Option<Resource>> + Send + 'static>>;

pub trait ResourceStorage: LoadResource {
	fn store(&mut self, data: &[u8]) -> ResourceHash;
	fn contains(&mut self, hash: &ResourceHash) -> bool;
	fn garbage_collect(&mut self, used: &[ResourceHash]);
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, PartialOrd, Ord, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceId(u64);

impl ResourceId {
	pub fn new() -> Self {
		Self(core_types::uuid::generate_uuid())
	}
}

impl std::fmt::Display for ResourceId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

pub type DataSources = Box<[DataSource]>;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DataSource {
	Embedded,
	Url(url::Url),
	Font { family: String, style: Option<String> },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceRegistry {
	hashes: HashMap<ResourceId, ResourceHash>,
	sources: HashMap<ResourceId, Vec<DataSource>>,
}

impl ResourceRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn is_empty(&self) -> bool {
		self.hashes.is_empty() && self.sources.is_empty()
	}

	pub fn contains(&self, id: &ResourceId) -> bool {
		self.hashes.contains_key(id) || self.sources.contains_key(id)
	}

	pub fn ids(&self) -> impl Iterator<Item = ResourceId> + '_ {
		self.hashes.keys().chain(self.sources.keys().filter(|id| !self.hashes.contains_key(id))).copied()
	}

	pub fn info(&self, id: &ResourceId) -> Option<ResourceInfo<'_>> {
		self.contains(id).then(|| ResourceInfo {
			id: *id,
			hash: self.hashes.get(id),
			sources: self.sources.get(id).map(|sources| sources.as_slice()).unwrap_or(&[]),
		})
	}

	pub fn push_source_back(&mut self, id: &ResourceId, source: DataSource) {
		self.sources.entry(*id).or_default().push(source);
	}

	pub fn push_source_front(&mut self, id: &ResourceId, source: DataSource) {
		self.sources.entry(*id).or_default().insert(0, source);
	}

	pub fn delete(&mut self, id: &ResourceId) -> bool {
		let hash = self.hashes.remove(id);
		let sources = self.sources.remove(id);
		!(hash.is_none() && sources.is_none())
	}

	pub fn resolve(&mut self, id: &ResourceId, hash: ResourceHash) -> Option<ResourceHash> {
		self.hashes.insert(*id, hash)
	}

	pub fn hash(&self, id: &ResourceId) -> Option<ResourceHash> {
		self.hashes.get(id).copied()
	}

	pub fn unresolved(&self) -> impl Iterator<Item = ResourceInfo<'_>> + '_ {
		self.sources.keys().filter(|id| !self.hashes.contains_key(id)).filter_map(|id| self.info(id))
	}

	pub fn resolved(&self) -> impl Iterator<Item = ResourceInfo<'_>> + '_ {
		self.hashes.keys().filter_map(|id| self.info(id))
	}
}

#[derive(Clone, Debug)]
pub struct ResourceInfo<'a> {
	pub id: ResourceId,
	pub hash: Option<&'a ResourceHash>,
	pub sources: &'a [DataSource],
}
