use super::identity::{anonymous_name, user_color};
use crate::application::generate_uuid;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::node_graph::utility_types::FrontendRemoteCursor;
use crate::messages::portfolio::document::utility_types::network_interface::TransactionStatus;
use crate::messages::portfolio::utility_types::PanelType;
use crate::messages::prelude::*;
use crate::messages::resource_storage::ResourcesHandle;
use crate::messages::viewport::Position;
use document_format::GddV1;
use document_graph_storage::{HistoryMetadata, Subject, UserId, user_attr};
use glam::{DAffine2, DVec2};
use graph_craft::application_io::resource::{LoadResource, ResourceHash};
use peer_transport::{CursorPosition, CursorSpace, DEFAULT_SIGNALING_SERVER, Event, Incoming, MessageLoopFuture, RemotePeer, Role, Room, SessionToken, SyncTarget, TransportPeerId};
use std::collections::HashSet;

/// How long a peer in a room without a host waits for one to greet it before taking the role itself.
const ROLE_GRACE_MS: f64 = 1_500.;
/// How long a document whose transport went down waits before it tries its room again.
const RECONNECT_AFTER_MS: f64 = 3_000.;

#[derive(ExtractField)]
pub struct SyncMessageContext<'a> {
	pub documents: &'a mut HashMap<DocumentId, DocumentMessageHandler>,
	pub active_document_id: Option<DocumentId>,
	pub resource_storage: &'a ResourceStorageMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub viewport: &'a ViewportMessageHandler,
	pub tools: &'a ToolMessageHandler,
}

/// Drives every document's collaborative session: connects them to their rooms, polls the rooms each frame and
/// whenever a packet arrives, and applies remote changes to the interface.
#[derive(Debug, Default, ExtractField)]
pub struct SyncMessageHandler {
	/// The documents being created to join a session link, each with its link.
	pending_joins: HashMap<DocumentId, SessionToken>,
	polling: bool,
	/// Each connected document's room inbox, with the generation of its connection.
	incoming: HashMap<DocumentId, (u32, Incoming)>,
	connections: u32,
	/// Documents with remote changes not yet applied to their interface.
	dirty: HashSet<DocumentId>,
	/// Documents that just synced with the host, whose viewport is fitted to the content once it is applied.
	fit_after_sync: HashSet<DocumentId>,
	/// When each document still without a host started waiting for one.
	undecided_since: HashMap<DocumentId, f64>,
	/// Why applying each dirty document waits, as last logged.
	blocked_reason: HashMap<DocumentId, String>,
	/// Held declarations being read from the byte store to be decoded.
	decoding: HashSet<(DocumentId, ResourceHash)>,
	/// The pointer position last sent for each connected document.
	last_cursor: HashMap<DocumentId, Option<CursorPosition>>,
	graph_cursors: Vec<FrontendRemoteCursor>,
	document_cursors: Vec<FrontendRemoteCursor>,
	/// When each document whose transport went down is next to be reconnected.
	reconnect_at: HashMap<DocumentId, f64>,
}

#[message_handler_data]
impl MessageHandler<SyncMessage, SyncMessageContext<'_>> for SyncMessageHandler {
	fn process_message(&mut self, message: SyncMessage, responses: &mut VecDeque<Message>, context: SyncMessageContext) {
		let SyncMessageContext {
			documents,
			active_document_id,
			resource_storage,
			preferences,
			ipp,
			viewport,
			tools,
		} = context;

		match message {
			SyncMessage::Share => {
				let Some(document_id) = active_document_id else { return };
				let Some(document) = documents.get_mut(&document_id) else { return };
				let Some(gdd) = document.storage() else {
					log::warn!("Cannot share a document before its working copy is mounted");
					return;
				};
				if gdd.role().is_some() {
					return;
				}
				document.stage_name_attribute();
				let Some(gdd) = document.storage_mut() else { return };
				self.host(document_id, gdd, preferences, responses);
			}
			SyncMessage::Join { token } => {
				let token = token.trim();
				if token.is_empty() {
					return;
				}
				let token = match token.parse::<SessionToken>() {
					Ok(token) => token,
					Err(error) => {
						log::warn!("Ignoring session link: {error}");
						return;
					}
				};
				let document_id = DocumentId(generate_uuid());
				self.pending_joins.insert(document_id, token);
				responses.add(PortfolioMessage::NewDocumentWithId {
					name: "Shared session".into(),
					document_id,
				});
			}
			SyncMessage::StorageMounted { document_id } => {
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};
				// Every document polls, shared or not, since retirement runs on the frame tick
				self.start_polling(responses);
				if let Some(token) = self.pending_joins.remove(&document_id) {
					let (room, driver) = Room::connect(&token.signaling_url(DEFAULT_SIGNALING_SERVER));
					let incoming = room.incoming();
					if let Err(error) = gdd.connect(room, UserId(preferences.user_id)) {
						log::error!("Joining the session failed: {error}");
						return;
					}
					announce_name(gdd, &preferences.user_name);
					self.attach(document_id, incoming, driver, responses);
					refresh_session_views(responses);
				} else if gdd.is_shared() && gdd.role().is_none() {
					self.connect_document(document_id, gdd, preferences, responses);
				} else {
					responses.add(SyncMessage::RefreshPanel);
				}
			}
			SyncMessage::Leave => {
				let Some(document_id) = active_document_id else { return };
				let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) else {
					return;
				};
				gdd.leave();
				self.detach(document_id);
				refresh_session_views(responses);
			}
			SyncMessage::Fork => {
				let Some(document_id) = active_document_id else { return };
				let Some(document) = documents.get_mut(&document_id) else { return };
				document.stage_name_attribute();
				let Some(gdd) = document.storage_mut() else { return };
				gdd.leave();
				self.detach(document_id);
				// The room derives from the manifest's document id, so a fresh id is a fresh room
				if let Err(error) = gdd.update_manifest(|manifest| manifest.document_id = generate_uuid()) {
					log::error!("Forking the session failed: {error}");
					refresh_session_views(responses);
					return;
				}
				self.host(document_id, gdd, preferences, responses);
			}
			SyncMessage::Disconnected { document_id, generation } => {
				if !self.is_current(document_id, generation) {
					return;
				}
				self.detach(document_id);
				// The document stays shared, so it reconnects after a pause and when it is reopened
				if let Some(gdd) = documents.get_mut(&document_id).and_then(|document| document.storage_mut()) {
					gdd.disconnect();
					self.reconnect_at.insert(document_id, now_ms() + RECONNECT_AFTER_MS);
				}
				refresh_session_views(responses);
			}
			SyncMessage::RefreshPanel => {
				let layout = session_panel_layout(documents, active_document_id, preferences);
				responses.add(LayoutMessage::SendLayout {
					layout,
					layout_target: LayoutTarget::SessionPanel,
				});
			}
			SyncMessage::Poll => {
				let cursor = active_cursor(documents, active_document_id, ipp, viewport, tools);
				self.poll(documents, active_document_id, resource_storage.resources_mut(), preferences, cursor, responses);
			}
			SyncMessage::Wake { document_id, generation } => {
				if !self.is_current(document_id, generation) {
					return;
				}
				let cursor = active_cursor(documents, active_document_id, ipp, viewport, tools);
				self.poll(documents, active_document_id, resource_storage.resources_mut(), preferences, cursor, responses);
				// The poll drained the inbox, so the next arrival wakes the next poll
				self.arm(document_id, responses);
			}
			SyncMessage::DeclarationLoaded { document_id, hash, bytes } => {
				let Some(bytes) = bytes else {
					// Left in `decoding` so it is not requested again every frame
					log::warn!("Declaration {hash} is recorded as held but the byte store has no bytes for it");
					return;
				};
				self.decoding.remove(&(document_id, hash));
				let Some(document) = documents.get_mut(&document_id) else { return };
				log::debug!("Decoded held declaration {hash} ({} bytes)", bytes.len());
				document.cache_declaration_bytes(hash, &bytes);
				self.dirty.insert(document_id);
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
		}
	}

	advertise_actions!(SyncMessageDiscriminant; Share, Leave);
}

impl SyncMessageHandler {
	/// Connects the document to its room, copies the join link to the clipboard and shows the Session panel.
	fn host(&mut self, document_id: DocumentId, gdd: &mut GddV1, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(token) = self.connect_document(document_id, gdd, preferences, responses) else { return };
		responses.add(FrontendMessage::TriggerSessionLinkCopy { token: token.to_string() });
		responses.add(WorkspaceMessage::FocusPanel { panel_type: PanelType::Session });
	}

	/// Connects the document's working copy to the room every copy of the document shares.
	fn connect_document(&mut self, document_id: DocumentId, gdd: &mut GddV1, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<SessionToken> {
		let token = SessionToken::for_document(gdd.manifest().document_id);
		let (room, driver) = Room::connect(&token.signaling_url(DEFAULT_SIGNALING_SERVER));
		let incoming = room.incoming();
		if let Err(error) = gdd.connect(room, UserId(preferences.user_id)) {
			log::error!("Connecting to the session failed: {error}");
			return None;
		}
		announce_name(gdd, &preferences.user_name);
		self.undecided_since.insert(document_id, now_ms());
		self.reconnect_at.remove(&document_id);
		self.attach(document_id, incoming, driver, responses);
		refresh_session_views(responses);
		Some(token)
	}

	/// Runs a new connection's transport and polls its room as packets arrive, not only per frame, since a hidden
	/// browser tab gets about one frame a second.
	fn attach(&mut self, document_id: DocumentId, incoming: Incoming, driver: MessageLoopFuture, responses: &mut VecDeque<Message>) {
		self.connections += 1;
		self.incoming.insert(document_id, (self.connections, incoming));
		self.arm(document_id, responses);
		responses.add(driver_future(document_id, self.connections, driver));
		self.start_polling(responses);
	}

	fn detach(&mut self, document_id: DocumentId) {
		self.incoming.remove(&document_id);
		self.last_cursor.remove(&document_id);
		self.reconnect_at.remove(&document_id);
		self.undecided_since.remove(&document_id);
	}

	/// Whether `generation` is the document's current connection.
	fn is_current(&self, document_id: DocumentId, generation: u32) -> bool {
		self.incoming.get(&document_id).is_some_and(|(current, _)| *current == generation)
	}

	/// Puts one wake future in flight for the document's current connection.
	fn arm(&mut self, document_id: DocumentId, responses: &mut VecDeque<Message>) {
		let Some((generation, incoming)) = self.incoming.get(&document_id).cloned() else { return };
		let future = async move {
			match incoming.wait().await {
				true => SyncMessage::Wake { document_id, generation }.into(),
				// The room's loop ended, and its driver reports the disconnection
				false => Message::NoOp,
			}
		};
		responses.add(future);
	}

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

	fn poll(
		&mut self,
		documents: &mut HashMap<DocumentId, DocumentMessageHandler>,
		active_document_id: Option<DocumentId>,
		resources: ResourcesHandle,
		preferences: &PreferencesMessageHandler,
		active_cursor: Option<CursorPosition>,
		responses: &mut VecDeque<Message>,
	) {
		for (&document_id, document) in documents.iter_mut() {
			let cursor = (Some(document_id) == active_document_id).then(|| active_cursor.clone()).flatten();
			self.poll_document(document_id, document, &resources, preferences, cursor, responses);
		}

		let active_document = active_document_id.and_then(|id| documents.get(&id));
		let graph_cursors = active_document.map(|document| graph_cursors(document, preferences)).unwrap_or_default();
		if graph_cursors != self.graph_cursors {
			self.graph_cursors = graph_cursors.clone();
			responses.add(FrontendMessage::UpdateNodeGraphCursors { cursors: graph_cursors });
		}
		let document_cursors = active_document.map(|document| document_cursors(document, preferences)).unwrap_or_default();
		if document_cursors != self.document_cursors {
			self.document_cursors = document_cursors.clone();
			responses.add(FrontendMessage::UpdateDocumentCursors { cursors: document_cursors });
		}

		self.apply_dirty(documents, active_document_id, &resources, responses);
	}

	fn poll_document(
		&mut self,
		document_id: DocumentId,
		document: &mut DocumentMessageHandler,
		resources: &ResourcesHandle,
		preferences: &PreferencesMessageHandler,
		cursor: Option<CursorPosition>,
		responses: &mut VecDeque<Message>,
	) {
		let idle = document.network_interface.transaction_status() == TransactionStatus::Finished;
		let Some(gdd) = document.storage_mut() else { return };
		if let Err(error) = gdd.retire_due(now_ms(), idle) {
			log::error!("Retirement failed: {error}");
		}
		record_profile(gdd, preferences);
		if self.reconnect_at.get(&document_id).is_some_and(|&due| now_ms() >= due) {
			self.reconnect_at.remove(&document_id);
			if gdd.is_shared() && gdd.role().is_none() {
				log::info!("Reconnecting {document_id:?} to its session");
				self.connect_document(document_id, gdd, preferences, responses);
			}
		}

		match gdd.role() {
			None => return,
			Some(Role::Undecided) => {
				let since = *self.undecided_since.entry(document_id).or_insert_with(now_ms);
				if now_ms() - since >= ROLE_GRACE_MS && gdd.decide_role().is_some() {
					self.undecided_since.remove(&document_id);
					refresh_session_views(responses);
				}
			}
			Some(_) => {
				self.undecided_since.remove(&document_id);
			}
		}

		// Broadcast the edits recorded since the last frame, so peers see each movement live
		document.stage_pending_edits(resources);
		let Some(gdd) = document.storage_mut() else { return };
		announce_name(gdd, &preferences.user_name);
		if self.last_cursor.get(&document_id) != Some(&cursor) {
			match gdd.send_cursor(cursor.clone()) {
				Ok(()) => {
					self.last_cursor.insert(document_id, cursor);
				}
				Err(error) => log::warn!("Sending the pointer position failed: {error}"),
			}
		}

		let events = gdd.poll_peers();
		// Taken on every poll, since a full sync can replace the registry without a change event
		let changes = gdd.take_remote_changes();
		if !changes.is_empty() {
			document.pending_remote.extend(changes);
			self.dirty.insert(document_id);
		}
		for event in events {
			match event {
				Event::Synced => {
					log::debug!("Synced with the host, applying its state");
					refresh_session_views(responses);
					self.dirty.insert(document_id);
					self.fit_after_sync.insert(document_id);
				}
				Event::Changed => {
					self.dirty.insert(document_id);
				}
				Event::ResourceRequested { from, hash } => {
					log::debug!("Peer asked for resource {hash}");
					responses.add(load_resource_future(document_id, from, hash, resources.clone()));
				}
				// The replica already put the bytes in the byte store, only the decoded declaration is missing
				Event::ResourceReceived { hash, bytes } => {
					log::debug!("Received resource {hash} ({} bytes)", bytes.len());
					document.cache_declaration_bytes(hash, &bytes);
					self.dirty.insert(document_id);
				}
				Event::RoleChanged { role } => {
					log::debug!("Session role is now {role:?}");
					refresh_session_views(responses);
				}
				Event::PeerJoined { .. } | Event::PeerLeft { .. } | Event::ProfileChanged { .. } => refresh_session_views(responses),
				Event::MetadataChanged => {
					refresh_session_views(responses);
					responses.add(HistoryMessage::Refresh);
				}
				// Cursors are gathered afresh on every poll
				Event::CursorMoved { .. } => {}
			}
		}
	}

	/// Applies the remote changes of each dirty document once every resource they reference is cached, and never
	/// under an open transaction, whose tool state may name nodes a peer removed.
	fn apply_dirty(&mut self, documents: &mut HashMap<DocumentId, DocumentMessageHandler>, active_document_id: Option<DocumentId>, resources: &ResourcesHandle, responses: &mut VecDeque<Message>) {
		for document_id in self.dirty.clone() {
			let Some(document) = documents.get_mut(&document_id) else {
				self.dirty.remove(&document_id);
				continue;
			};
			let Some(gdd) = document.storage() else { continue };
			let missing = SyncTarget::missing_resources(gdd).len();
			let transaction = document.network_interface.transaction_status();
			if missing > 0 || transaction != TransactionStatus::Finished {
				self.wait(document_id, format!("missing {missing} transaction {transaction:?}"));
				continue;
			}
			// A declaration held since an earlier session was never received from a peer, so it is read from the byte store to be decoded
			let undecoded = document.undecoded_declaration_hashes();
			if !undecoded.is_empty() {
				self.wait(document_id, format!("decoding {} held declarations", undecoded.len()));
				for hash in undecoded {
					if self.decoding.insert((document_id, hash)) {
						responses.add(decode_declaration_future(document_id, hash, resources.clone()));
					}
				}
				continue;
			}
			self.blocked_reason.remove(&document_id);

			if document.apply_remote_changes(responses) {
				self.dirty.remove(&document_id);
				if document.adopt_name_from_storage() {
					responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				}
				if self.fit_after_sync.remove(&document_id) && Some(document_id) == active_document_id {
					// The bounds come from the render, so the fit waits for the graph to run
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![DocumentMessage::ZoomCanvasToFitAll.into()],
					});
				}
			}
			responses.add(PortfolioMessage::ResolveDocumentResources { document_id });
		}
	}

	fn wait(&mut self, document_id: DocumentId, reason: String) {
		if self.blocked_reason.get(&document_id) != Some(&reason) {
			log::debug!("Applying remote changes to {document_id:?} waits: {reason}");
			self.blocked_reason.insert(document_id, reason);
		}
	}
}

fn driver_future(document_id: DocumentId, generation: u32, driver: MessageLoopFuture) -> Message {
	let future = async move {
		if let Err(error) = driver.await {
			log::warn!("Session transport for {document_id:?} ended: {error}");
		}
		SyncMessage::Disconnected { document_id, generation }.into()
	};
	future.into()
}

fn decode_declaration_future(document_id: DocumentId, hash: ResourceHash, resources: ResourcesHandle) -> Message {
	let future = async move {
		let bytes = resources.load(hash).await.map(|resource| resource.as_ref().to_vec());
		SyncMessage::DeclarationLoaded { document_id, hash, bytes }.into()
	};
	future.into()
}

fn load_resource_future(document_id: DocumentId, to: TransportPeerId, hash: ResourceHash, resources: ResourcesHandle) -> Message {
	let future = async move {
		let Some(resource) = resources.load(hash).await else {
			log::warn!("A peer asked for resource {hash} which is not in the byte store");
			return Message::NoOp;
		};
		SyncMessage::ResourceLoaded {
			document_id,
			to,
			hash,
			bytes: resource.as_ref().to_vec(),
		}
		.into()
	};
	future.into()
}

/// Wall-clock milliseconds since the Unix epoch.
pub(crate) fn now_ms() -> f64 {
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

/// Updates the session circle on the document tabs and the Session panel.
fn refresh_session_views(responses: &mut VecDeque<Message>) {
	responses.add(PortfolioMessage::UpdateOpenDocumentsList);
	responses.add(SyncMessage::RefreshPanel);
}

fn session_panel_layout(documents: &HashMap<DocumentId, DocumentMessageHandler>, active_document_id: Option<DocumentId>, preferences: &PreferencesMessageHandler) -> Layout {
	let heading = |text: &str| LayoutGroup::row(vec![TextLabel::new(text).bold(true).widget_instance()]);
	let note = |text: &str| LayoutGroup::row(vec![TextLabel::new(text).multiline(true).widget_instance()]);
	let user_name = preferences.user_name.as_str();
	let name_row = || {
		LayoutGroup::row(vec![
			TextLabel::new("Your name").table_align(true).sizing(WidgetSizing::fixed(90)).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(user_name)
				.placeholder(anonymous_name(UserId(preferences.user_id)))
				.on_update(|input: &TextInput| PreferencesMessage::UserName { name: input.value.clone() }.into())
				.widget_instance(),
		])
	};
	let cursors_row = || {
		let checkbox_id = CheckboxId::new();
		LayoutGroup::row(vec![
			TextLabel::new("Show cursors")
				.table_align(true)
				.sizing(WidgetSizing::fixed(90))
				.for_checkbox(checkbox_id)
				.widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			CheckboxInput::new(preferences.show_remote_cursors)
				.tooltip_description("Draw the other peers' pointers over the document.")
				.on_update(|checkbox: &CheckboxInput| PreferencesMessage::ShowRemoteCursors { enabled: checkbox.checked }.into())
				.for_label(checkbox_id)
				.widget_instance(),
		])
	};

	let Some(document) = active_document_id.and_then(|id| documents.get(&id)) else {
		return Layout(vec![note("Open a document to share it.")]);
	};
	let Some(gdd) = document.storage() else {
		return Layout(vec![name_row(), cursors_row(), note("The document's working copy is still being mounted.")]);
	};

	let fork = || {
		TextButton::new("Fork")
			.tooltip_label("Fork the session")
			.tooltip_description("Leave this room and host the document as a new session with its own link. The others keep the room they are in.")
			.on_commit(|_| SyncMessage::Fork.into())
			.widget_instance()
	};
	let (state, actions, detail) = match gdd.role() {
		None if gdd.is_shared() => (
			"Disconnected, reconnecting",
			vec![
				TextButton::new("Reconnect").icon("Link").emphasized(true).on_commit(|_| SyncMessage::Share.into()).widget_instance(),
				TextButton::new("Stop Sharing").on_commit(|_| SyncMessage::Leave.into()).widget_instance(),
				fork(),
			],
			note("The connection to the session was lost. The editor keeps trying to reconnect, and the document stays editable meanwhile."),
		),
		None => (
			"Not shared",
			vec![
				TextButton::new("Share Live Session")
					.icon("Link")
					.emphasized(true)
					.on_commit(|_| SyncMessage::Share.into())
					.widget_instance(),
			],
			LayoutGroup::column(vec![
				TextLabel::new("Share this document to edit it live with others. Everyone who opens the link works on the same document.")
					.multiline(true)
					.widget_instance(),
			]),
		),
		Some(role) => {
			let live = role == Role::Host || (role == Role::Guest && gdd.is_synced());
			let token = SessionToken::for_document(gdd.manifest().document_id).to_string();
			let copy_link_token = token.clone();
			let copy_token = token.clone();
			(
				if live { "Live session" } else { "Connecting" },
				vec![
					TextButton::new("Copy Link")
						.icon("Copy")
						.emphasized(true)
						.on_commit(move |_| FrontendMessage::TriggerSessionLinkCopy { token: copy_link_token.clone() }.into())
						.widget_instance(),
					TextButton::new("Disconnect").on_commit(|_| SyncMessage::Leave.into()).widget_instance(),
					fork(),
				],
				LayoutGroup::row(vec![
					TextLabel::new(token)
						.monospace(true)
						.selectable(true)
						.tooltip_label("Session")
						.tooltip_description("Opening the editor with this session in the link joins the room.")
						.widget_instance(),
					IconButton::new("Copy", 24)
						.tooltip_label("Copy Session Token")
						.on_update(move |_| FrontendMessage::TriggerClipboardWrite { content: copy_token.clone() }.into())
						.widget_instance(),
				]),
			)
		}
	};
	let mut groups = vec![name_row(), cursors_row(), heading(state), LayoutGroup::row(actions), detail];
	if gdd.role().is_none() && !gdd.is_shared() {
		groups.push(LayoutGroup::row(vec![
			TextLabel::new("Join a session").table_align(true).sizing(WidgetSizing::fixed(90)).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new("")
				.placeholder("Paste a link")
				.tooltip_description("Opens the shared document in a new tab, following the session behind the link.")
				.on_update(|input: &TextInput| SyncMessage::Join { token: input.value.clone() }.into())
				.widget_instance(),
		]));
	}

	if gdd.role().is_some() {
		let mut peers: Vec<_> = gdd.peers().collect();
		peers.sort_by_key(|remote| remote.peer);
		groups.push(heading(&format!("In the session ({})", peers.len() + 1)));
		let me = match user_name.is_empty() {
			true => anonymous_name(UserId(preferences.user_id)),
			false => user_name.to_string(),
		};
		groups.push(peer_row(&format!("{me} (you)"), user_name.is_empty()));
		let metadata = gdd.metadata();
		groups.extend(peers.iter().map(|remote| peer_row(&display_name(remote, metadata), is_anonymous(remote, metadata))));
	}
	Layout(groups)
}

/// An anonymous peer's stand-in name is shown in italics.
fn peer_row(name: &str, anonymous: bool) -> LayoutGroup {
	LayoutGroup::row(vec![TextLabel::new(name).italic(anonymous).widget_instance()])
}

/// Puts this user's display name on the document's record of its users. An empty name is recorded only to clear one.
fn record_profile(gdd: &mut GddV1, preferences: &PreferencesMessageHandler) {
	let name = preferences.user_name.trim();
	let user = UserId(preferences.user_id);
	if gdd.metadata().user_name(user).unwrap_or_default() == name {
		return;
	}
	if let Err(error) = gdd.record(Subject::User(user), user_attr::NAME, serde_json::Value::from(name), now_ms()) {
		log::warn!("Recording the display name failed: {error}");
	}
}

fn announce_name(gdd: &mut GddV1, name: &str) {
	if let Err(error) = gdd.set_name(name) {
		log::warn!("Announcing the display name failed: {error}");
	}
}

fn active_cursor(
	documents: &HashMap<DocumentId, DocumentMessageHandler>,
	active_document_id: Option<DocumentId>,
	ipp: &InputPreprocessorMessageHandler,
	viewport: &ViewportMessageHandler,
	tools: &ToolMessageHandler,
) -> Option<CursorPosition> {
	let document = documents.get(&active_document_id?)?;
	cursor_position(document, ipp, viewport, tools)
}

/// The pointer while it is over the viewport, in the node graph's network when the graph is open and in the document
/// otherwise. Rounded to a hundredth of a unit, so jitter below that sends nothing.
fn cursor_position(document: &DocumentMessageHandler, ipp: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler, tools: &ToolMessageHandler) -> Option<CursorPosition> {
	let mouse = ipp.mouse.position;
	let size = viewport.size();
	if mouse.x < 0. || mouse.y < 0. || mouse.x > size.x() || mouse.y > size.y() {
		return None;
	}
	let (to_viewport, space) = match document.is_graph_overlay_open() {
		true => (graph_to_viewport(document)?, CursorSpace::Graph { network: shown_network(document) }),
		false => (document.metadata().document_to_viewport, CursorSpace::Document),
	};
	let position = to_viewport.inverse().transform_point2(mouse);
	if !position.is_finite() {
		return None;
	}
	let tool = tools.active_tool_icon();
	Some(CursorPosition {
		position: [(position.x * 100.).round() / 100., (position.y * 100.).round() / 100.],
		space,
		tool: (!tool.is_empty()).then_some(tool),
	})
}

/// The other peers' pointers over the document while the canvas is showing.
fn document_cursors(document: &DocumentMessageHandler, preferences: &PreferencesMessageHandler) -> Vec<FrontendRemoteCursor> {
	if !preferences.show_remote_cursors || document.is_graph_overlay_open() {
		return Vec::new();
	}
	let Some(gdd) = document.storage().filter(|gdd| gdd.role().is_some()) else { return Vec::new() };
	let to_viewport = document.metadata().document_to_viewport;
	gdd.peers()
		.filter_map(|remote| {
			let cursor = remote.cursor.as_ref().filter(|cursor| matches!(cursor.space, CursorSpace::Document))?;
			remote_cursor(remote, cursor, to_viewport, gdd.metadata())
		})
		.collect()
}

/// The other peers' pointers over the network the node graph shows.
fn graph_cursors(document: &DocumentMessageHandler, preferences: &PreferencesMessageHandler) -> Vec<FrontendRemoteCursor> {
	if !preferences.show_remote_cursors || !document.is_graph_overlay_open() {
		return Vec::new();
	}
	let Some(gdd) = document.storage().filter(|gdd| gdd.role().is_some()) else { return Vec::new() };
	let Some(to_viewport) = graph_to_viewport(document) else { return Vec::new() };
	let shown = shown_network(document);
	gdd.peers()
		.filter_map(|remote| {
			let cursor = remote.cursor.as_ref()?;
			let CursorSpace::Graph { network } = &cursor.space else { return None };
			if *network != shown {
				return None;
			}
			remote_cursor(remote, cursor, to_viewport, gdd.metadata())
		})
		.collect()
}

/// A peer's pointer in viewport pixels, `None` when the position it sent is not a finite point.
fn remote_cursor(remote: &RemotePeer, cursor: &CursorPosition, to_viewport: DAffine2, metadata: &HistoryMetadata) -> Option<FrontendRemoteCursor> {
	let [x, y] = cursor.position;
	let point = to_viewport.transform_point2(DVec2::new(x, y));
	point.is_finite().then(|| FrontendRemoteCursor {
		x: point.x,
		y: point.y,
		name: display_name(remote, metadata),
		anonymous: is_anonymous(remote, metadata),
		color: user_color(remote.user),
		tool: cursor.tool.clone(),
	})
}

fn graph_to_viewport(document: &DocumentMessageHandler) -> Option<DAffine2> {
	let metadata = document.network_interface.network_metadata(document.breadcrumb_network_path())?;
	Some(metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport)
}

fn shown_network(document: &DocumentMessageHandler) -> Vec<u64> {
	document.breadcrumb_network_path().iter().map(|id| id.0).collect()
}

/// The document's record of the user, else the name the peer announced to the room, else the stand-in.
fn display_name(remote: &RemotePeer, metadata: &HistoryMetadata) -> String {
	if let Some(name) = metadata.user_name(remote.user) {
		return name.to_string();
	}
	match remote.name.is_empty() {
		true => anonymous_name(remote.user),
		false => remote.name.clone(),
	}
}

fn is_anonymous(remote: &RemotePeer, metadata: &HistoryMetadata) -> bool {
	metadata.user_name(remote.user).is_none() && remote.name.is_empty()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_second_join_does_not_drop_the_first() {
		let mut sync = SyncMessageHandler::default();
		let (resource_storage, preferences, ipp, viewport, tools) = Default::default();
		let mut documents = HashMap::new();
		for byte in [1, 2] {
			let context = SyncMessageContext {
				documents: &mut documents,
				active_document_id: None,
				resource_storage: &resource_storage,
				preferences: &preferences,
				ipp: &ipp,
				viewport: &viewport,
				tools: &tools,
			};
			let token = SessionToken([byte; 16]).to_string();
			sync.process_message(SyncMessage::Join { token }, &mut VecDeque::new(), context);
		}

		let mut tokens: Vec<_> = sync.pending_joins.values().copied().collect();
		tokens.sort_by_key(|token| token.0);
		assert_eq!(tokens, [SessionToken([1; 16]), SessionToken([2; 16])]);
	}
}
