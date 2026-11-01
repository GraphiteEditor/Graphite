use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::network_interface::TransactionStatus;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct SyncMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
}

/// Retires what is due in every mounted document, once per frame.
#[derive(Debug, Default, ExtractField)]
pub struct SyncMessageHandler {
	polling: bool,
}

#[message_handler_data]
impl MessageHandler<SyncMessage, SyncMessageContext<'_>> for SyncMessageHandler {
	fn process_message(&mut self, message: SyncMessage, responses: &mut VecDeque<Message>, context: SyncMessageContext) {
		let SyncMessageContext { documents } = context;

		match message {
			SyncMessage::StorageMounted => {
				if self.polling {
					return;
				}
				self.polling = true;
				responses.add(BroadcastMessage::SubscribeEvent {
					on: EventMessage::AnimationFrame,
					send: Box::new(SyncMessage::Poll.into()),
				});
			}
			SyncMessage::Poll => {
				for document in documents.values_mut() {
					let idle = document.network_interface.transaction_status() == TransactionStatus::Finished;
					let Some(gdd) = document.storage_mut() else { continue };
					if let Err(error) = gdd.retire_due(now_ms(), idle) {
						log::error!("Retirement failed: {error}");
					}
				}
			}
		}
	}

	advertise_actions!(SyncMessageDiscriminant;);
}

fn now_ms() -> f64 {
	#[cfg(target_arch = "wasm32")]
	{
		js_sys::Date::now()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|elapsed| elapsed.as_secs_f64() * 1000.)
			.unwrap_or(0.)
	}
}
