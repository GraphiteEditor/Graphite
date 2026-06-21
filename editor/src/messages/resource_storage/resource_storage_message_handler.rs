use crate::messages::prelude::*;
use graph_craft::application_io::resource::{LoadResource, ResourceFuture, ResourceHash, ResourceStorage};
use std::sync::Arc;

#[derive(Clone)]
pub struct ResourcesHandle {
	inner: Arc<dyn ResourceStorage>,
}

impl LoadResource for ResourcesHandle {
	fn load(&self, hash: ResourceHash) -> ResourceFuture<'_> {
		self.inner.load(hash)
	}
}

impl ResourceStorage for ResourcesHandle {
	fn store(&self, data: &[u8]) -> ResourceHash {
		self.inner.store(data)
	}

	fn contains(&self, hash: &ResourceHash) -> bool {
		self.inner.contains(hash)
	}

	fn garbage_collect(&self, used: &[ResourceHash]) {
		self.inner.garbage_collect(used)
	}
}

#[derive(ExtractField)]
pub struct ResourceStorageMessageHandler {
	storage: Option<Arc<dyn ResourceStorage>>,
}

impl ResourceStorageMessageHandler {
	pub fn new(resource_storage: Arc<dyn ResourceStorage>) -> Self {
		Self { storage: Some(resource_storage) }
	}

	pub fn resources(&self) -> Box<dyn LoadResource> {
		Box::new(ResourcesHandle {
			inner: self.storage.clone().expect("Resource storage not initialized"),
		})
	}

	/// The backing store as a `&dyn ResourceStorage`, for write paths (e.g. persisting declaration
	/// bytes on commit). `None` before initialization.
	pub fn storage(&self) -> Option<&dyn ResourceStorage> {
		self.storage.as_deref()
	}

	/// An owned, cloneable handle that both loads and stores, for `'static` async tasks that need to
	/// read and write the cache off-thread. `None` before initialization.
	pub fn store_handle(&self) -> Option<ResourcesHandle> {
		self.storage.clone().map(|inner| ResourcesHandle { inner })
	}
}

impl std::fmt::Debug for ResourceStorageMessageHandler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ResourceStorageMessageHandler").finish_non_exhaustive()
	}
}

impl Default for ResourceStorageMessageHandler {
	#[cfg(not(test))]
	fn default() -> Self {
		Self { storage: None }
	}

	#[cfg(test)]
	fn default() -> Self {
		Self {
			storage: Some(Arc::new(graph_craft::application_io::resource::HashMapResourceStorage::new())),
		}
	}
}

#[derive(ExtractField)]
pub struct ResourceStorageMessageContext {}

#[message_handler_data]
impl MessageHandler<ResourceStorageMessage, ResourceStorageMessageContext> for ResourceStorageMessageHandler {
	fn process_message(&mut self, message: ResourceStorageMessage, _responses: &mut VecDeque<Message>, _context: ResourceStorageMessageContext) {
		let Some(storage) = &self.storage else {
			log::error!("Received resource message but storage is not initialized");
			return;
		};

		match message {
			ResourceStorageMessage::Store { data } => {
				let _hash = storage.store(data.as_ref());
			}
			ResourceStorageMessage::GarbageCollect { used } => {
				storage.garbage_collect(&used);
			}
		}
	}

	advertise_actions!(ResourceStorageMessageDiscriminant;);
}
