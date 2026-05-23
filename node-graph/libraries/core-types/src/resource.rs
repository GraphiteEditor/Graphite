use dyn_any::DynAny;
use graphene_hash::CacheHash;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, DynAny)]
pub struct Resource {
	inner: Arc<dyn AsRef<[u8]> + Send + Sync>,
}

impl Resource {
	pub fn new<T: AsRef<[u8]> + Send + Sync + 'static>(data: T) -> Self {
		Self { inner: Arc::new(data) }
	}
}

impl From<&Resource> for Arc<dyn AsRef<[u8]> + Send + Sync> {
	fn from(val: &Resource) -> Self {
		val.inner.clone()
	}
}

impl Deref for Resource {
	type Target = [u8];

	fn deref(&self) -> &[u8] {
		(*self.inner).as_ref()
	}
}

impl AsRef<[u8]> for Resource {
	fn as_ref(&self) -> &[u8] {
		(*self.inner).as_ref()
	}
}

impl std::fmt::Debug for Resource {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Resource").field("len", &self.len()).finish()
	}
}

impl PartialEq for Resource {
	fn eq(&self, other: &Self) -> bool {
		self.as_ref() == other.as_ref()
	}
}

impl CacheHash for Resource {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.as_ref().hash(state);
	}
}
