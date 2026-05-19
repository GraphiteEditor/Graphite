use crate::messages::prelude::*;
use graph_craft::application_io::{ResourceFuture, ResourceHash, ResourceStorage, Resources};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct ResourcesHandle {
	inner: Arc<RwLock<Box<dyn ResourceStorage>>>,
}

impl Resources for ResourcesHandle {
	fn load(&self, hash: ResourceHash) -> ResourceFuture {
		let guard = self.inner.read().unwrap();
		guard.load(hash)
	}
}

#[derive(ExtractField)]
pub struct ResourceMessageHandler {
	storage: Option<Arc<RwLock<Box<dyn ResourceStorage>>>>,
}

impl ResourceMessageHandler {
	pub fn new(resource_storage: Box<dyn ResourceStorage>) -> Self {
		Self {
			storage: Some(Arc::new(RwLock::new(resource_storage))),
		}
	}

	pub fn resources(&self) -> Box<dyn Resources> {
		Box::new(ResourcesHandle {
			inner: self.storage.clone().expect("Resource storage not initialized"),
		})
	}
}

impl std::fmt::Debug for ResourceMessageHandler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ResourceMessageHandler").finish_non_exhaustive()
	}
}

impl Default for ResourceMessageHandler {
	#[cfg(not(test))]
	fn default() -> Self {
		Self { storage: None }
	}

	#[cfg(test)]
	fn default() -> Self {
		Self {
			storage: Some(Arc::new(RwLock::new(Box::new(graph_craft::application_io::HashMapResourceStorage::new())))),
		}
	}
}

#[derive(ExtractField)]
pub struct ResourceMessageContext {}

#[message_handler_data]
impl MessageHandler<ResourceMessage, ResourceMessageContext> for ResourceMessageHandler {
	fn process_message(&mut self, message: ResourceMessage, _responses: &mut VecDeque<Message>, _context: ResourceMessageContext) {
		let Some(storage) = &self.storage else {
			log::error!("Received resource message but storage is not initialized");
			return;
		};
		let mut storage = storage.write().unwrap();

		match message {
			ResourceMessage::Write { data } => {
				let _hash = storage.write(data.as_ref());
			}
			ResourceMessage::GarbageCollect { used } => {
				storage.garbage_collect(&used);
			}
		}
	}

	advertise_actions!(ResourceMessageDiscriminant;);
}
