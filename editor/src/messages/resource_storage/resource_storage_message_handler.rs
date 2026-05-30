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
