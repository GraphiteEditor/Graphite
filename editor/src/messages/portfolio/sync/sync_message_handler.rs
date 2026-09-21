use crate::application::generate_uuid;
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::network_interface::TransactionStatus;
use crate::messages::portfolio::document_storage_io::rebuild_interface;
use crate::messages::prelude::*;
use crate::messages::resource_storage::ResourcesHandle;
use document_graph_storage::UserId;
use graph_craft::application_io::resource::{LoadResource, ResourceStorage};
use peer_transport::{DEFAULT_SIGNALING_SERVER, Event, Room, SessionToken, SyncTarget};
use std::collections::HashSet;

#[derive(ExtractField)]
pub struct SyncMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	pub resource_storage: &'a ResourceStorageMessageHandler,
}

/// Drives every document's collaborative session: attaches transports, polls them once per frame,
/// and turns remote changes into interface rebuilds.
#[derive(Debug, Default, ExtractField)]
pub struct SyncMessageHandler {
	pending_join: Option<SessionToken>,
	polling: bool,
	/// Documents whose registry changed remotely since their interface was last rebuilt.
	dirty: HashSet<DocumentId>,
	rebuilding: HashSet<DocumentId>,
	/// Received resources still being copied into the app cache, per document.
	storing: HashMap<DocumentId, usize>,
	blocked_reason: HashMap<DocumentId, String>,
}

#[message_handler_data]
impl MessageHandler<SyncMessage, SyncMessageContext<'_>> for SyncMessageHandler {
	fn process_message(&mut self, message: SyncMessage, responses: &mut VecDeque<Message>, context: SyncMessageContext) {
		let SyncMessageContext {
			documents,
			active_document_id,
			resource_storage,
		} = context;

		match message {
			SyncMessage::Share => {
				let Some(document) = active_document_id.and_then(|id| documents.get_mut(&id)) else { return };
				let Some(gdd) = document.storage_mut() else {
					log::warn!("Cannot share a document before its working copy is mounted");
					return;
				};
				if gdd.role().is_some() {
					return;
				}

				let token = SessionToken(random_token_bytes());
				let (room, driver) = Room::connect(&token.signaling_url(DEFAULT_SIGNALING_SERVER));
				let user = UserId(gdd.session().peer().0);
				gdd.share(room, user);

				let document_id = active_document_id.expect("checked above");
				responses.add(driver_future(document_id, driver));
				responses.add(FrontendMessage::TriggerSessionLinkCopy { token: token.to_string() });
				responses.add(DialogMessage::DisplayDialogError {
					title: "Live session started".into(),
					description: format!("A link to join was copied to the clipboard.\n\nSession {token}"),
				});
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				self.start_polling(responses);
			}
			SyncMessage::Join { token } => {
				let token = match token.parse::<SessionToken>() {
					Ok(token) => token,
					Err(error) => {
						log::warn!("Ignoring session link: {error}");
						return;
					}
				};
				self.pending_join = Some(token);
				responses.add(PortfolioMessage::NewDocumentWithName { name: "Shared session".into() });
			}
			SyncMessage::StorageMounted { document_id } => {
				let Some(token) = self.pending_join.take() else { return };
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};

				let (room, driver) = Room::connect(&token.signaling_url(DEFAULT_SIGNALING_SERVER));
				let user = UserId(gdd.session().peer().0);
				gdd.join(room, user);

				responses.add(driver_future(document_id, driver));
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				self.start_polling(responses);
			}
			SyncMessage::Leave => {
				let Some(gdd) = active_document_id.and_then(|id| documents.get_mut(&id)).and_then(|document| document.storage_mut()) else {
					return;
				};
				gdd.leave();
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			SyncMessage::Disconnected { document_id } => {
				if let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) {
					gdd.leave();
				}
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			SyncMessage::Poll => {
				let resources = resource_storage.resources_mut();
				for (&document_id, document) in documents.iter_mut() {
					let Some(gdd) = document.storage_mut() else { continue };
					if gdd.role().is_none() {
						continue;
					}

					let events = gdd.poll_peers();
					let proxy = gdd.resource_proxy();
					for event in events {
						match event {
							Event::Synced | Event::Changed => {
								self.dirty.insert(document_id);
								document.runtime_stale = true;
							}
							Event::ResourceRequested { from, hash } => {
								log::debug!("Peer asked for resource {hash}");
								responses.add(load_resource_future(document_id, from, hash, proxy.clone(), resources.clone()));
							}
							Event::ResourceReceived(hash) => {
								log::debug!("Received resource {hash}");
								*self.storing.entry(document_id).or_default() += 1;
								responses.add(store_resource_future(document_id, hash, proxy.clone(), resources.clone()));
							}
							Event::PeerJoined { .. } | Event::PeerLeft { .. } => responses.add(PortfolioMessage::UpdateOpenDocumentsList),
						}
					}
				}

				// One rebuild in flight per document, only once every referenced resource is in the app cache,
				// and never underneath an open transaction, whose un-staged edits the swap would discard.
				for document_id in self.dirty.clone() {
					let Some(document) = documents.get(&document_id) else {
						self.dirty.remove(&document_id);
						continue;
					};
					let Some(gdd) = document.storage() else { continue };
					let storing = self.storing.get(&document_id).copied().unwrap_or(0);
					let missing = SyncTarget::missing_resources(gdd).len();
					let transaction = document.network_interface.transaction_status();
					if self.rebuilding.contains(&document_id) || storing > 0 || missing > 0 || transaction != TransactionStatus::Finished {
						let reason = format!("rebuilding {} storing {storing} missing {missing} transaction {transaction:?}", self.rebuilding.contains(&document_id));
						if self.blocked_reason.get(&document_id) != Some(&reason) {
							log::debug!("Sync rebuild for {document_id:?} waiting: {reason}");
							self.blocked_reason.insert(document_id, reason);
						}
						continue;
					}
					self.blocked_reason.remove(&document_id);

					self.dirty.remove(&document_id);
					self.rebuilding.insert(document_id);
					responses.add(rebuild_future(document_id, gdd.clone(), resources.clone()));
				}
			}
			SyncMessage::Rebuilt { document_id, interface } => {
				log::debug!("Sync rebuild for {document_id:?} finished: {}", if interface.is_some() { "applying" } else { "failed" });
				self.rebuilding.remove(&document_id);
				let Some(document) = documents.get_mut(&document_id) else { return };
				let Some(interface) = interface.map(|boxed| *boxed) else { return };
				document.apply_gdd_cursor_rebuild(interface, false, false, responses);
			}
			SyncMessage::ResourceLoaded { document_id, to, hash, bytes } => {
				log::debug!("Sending resource {hash} ({} bytes)", bytes.len());
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};
				if let Err(error) = gdd.send_resource(to, hash, bytes) {
					log::warn!("Failed to send resource {hash} to a peer: {error}");
				}
			}
			SyncMessage::ResourceStored { document_id } => {
				if let Some(count) = self.storing.get_mut(&document_id) {
					*count = count.saturating_sub(1);
				}
				self.dirty.insert(document_id);
			}
		}
	}

	advertise_actions!(SyncMessageDiscriminant; Share, Leave);
}

impl SyncMessageHandler {
	fn start_polling(&mut self, responses: &mut VecDeque<Message>) {
		if self.polling {
			return;
		}
		self.polling = true;
		responses.add(BroadcastMessage::SubscribeEvent {
			on: EventMessage::AnimationFrame,
			send: Box::new(SyncMessage::Poll.into()),
		});
	}
}

fn driver_future(document_id: DocumentId, driver: peer_transport::MessageLoopFuture) -> Message {
	let future = async move {
		if let Err(error) = driver.await {
			log::warn!("Session transport for {document_id:?} ended: {error}");
		}
		Message::Portfolio(PortfolioMessage::Sync(SyncMessage::Disconnected { document_id }))
	};
	future.into()
}

fn rebuild_future(document_id: DocumentId, gdd: document_format::GddV1, resources: ResourcesHandle) -> Message {
	let future = async move {
		let interface = rebuild_interface(&gdd, &resources, document_id).await.map(Box::new);
		Message::Portfolio(PortfolioMessage::Sync(SyncMessage::Rebuilt { document_id, interface }))
	};
	future.into()
}

/// The working copy is checked before the app cache: it is the document's own store, which another tab
/// sharing the cache can't garbage-collect from under it.
fn load_resource_future(
	document_id: DocumentId,
	to: peer_transport::TransportPeerId,
	hash: graph_craft::application_io::resource::ResourceHash,
	proxy: document_format::ResourceProxy<document_format::GddV1Layout>,
	resources: ResourcesHandle,
) -> Message {
	let future = async move {
		let resource = match proxy.load(hash).await {
			Some(resource) => Some(resource),
			None => resources.load(hash).await,
		};
		match resource {
			Some(resource) => Message::Portfolio(PortfolioMessage::Sync(SyncMessage::ResourceLoaded {
				document_id,
				to,
				hash,
				bytes: resource.as_ref().to_vec(),
			})),
			None => {
				log::warn!("A peer asked for resource {hash} which is not in the byte store");
				Message::NoOp
			}
		}
	};
	future.into()
}

fn store_resource_future(
	document_id: DocumentId,
	hash: graph_craft::application_io::resource::ResourceHash,
	proxy: document_format::ResourceProxy<document_format::GddV1Layout>,
	resources: ResourcesHandle,
) -> Message {
	let future = async move {
		match proxy.load(hash).await {
			Some(resource) => {
				resources.store(resource.as_ref());
			}
			None => log::warn!("Received resource {hash} is missing from the working copy"),
		}
		Message::Portfolio(PortfolioMessage::Sync(SyncMessage::ResourceStored { document_id }))
	};
	future.into()
}

fn random_token_bytes() -> [u8; 16] {
	let mut bytes = [0; 16];
	bytes[..8].copy_from_slice(&generate_uuid().to_le_bytes());
	bytes[8..].copy_from_slice(&generate_uuid().to_le_bytes());
	bytes
}
