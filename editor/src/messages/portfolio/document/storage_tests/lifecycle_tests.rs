//! Document store lifecycle: discovery, session restore, legacy/storage reconciliation, mode changes, and removal.

use crate::messages::frontend::utility_types::PersistedState;
use crate::messages::portfolio::document_storage_io::document_key;
use crate::messages::prelude::*;
use crate::test_utils::EditorTestUtils;
use document_container::{AnyContainer, AsyncContainer};

fn dispatch(editor: &mut EditorTestUtils, message: impl Into<Message>) -> Vec<FrontendMessage> {
	editor.editor.handle_message(message)
}

fn portfolio(editor: &EditorTestUtils) -> &PortfolioMessageHandler {
	&editor.editor.dispatcher.message_handlers.portfolio_message_handler
}

fn restart(editor: &mut EditorTestUtils, state: PersistedState) {
	dispatch(editor, PortfolioMessage::DestroyAllDocuments);
	dispatch(editor, PersistentStateMessage::LoadState { state });
}

fn new_document(editor: &mut EditorTestUtils, name: &str) -> DocumentId {
	dispatch(editor, PortfolioMessage::NewDocumentWithName { name: name.into() });
	portfolio(editor).active_document_id.unwrap()
}

async fn container(editor: &EditorTestUtils, document_id: DocumentId) -> AnyContainer {
	portfolio(editor).document_store().open(document_key(document_id)).await.unwrap()
}

async fn stored_ids(editor: &EditorTestUtils) -> Vec<DocumentId> {
	portfolio(editor).document_store().list().await.unwrap().into_iter().map(|key| DocumentId(key.0)).collect()
}

#[tokio::test]
async fn stored_documents_are_discovered_without_session_state() {
	let mut editor = EditorTestUtils::create();
	let one = new_document(&mut editor, "One");
	let two = new_document(&mut editor, "Two");

	restart(&mut editor, PersistedState::default());

	let state = portfolio(&editor).persisted_state_snapshot();
	let mut ids: Vec<_> = state.documents.iter().map(|info| info.id).collect();
	ids.sort();
	let mut expected = vec![one, two];
	expected.sort();
	assert_eq!(ids, expected);
	assert_ne!(
		state.documents[0].name, state.documents[1].name,
		"names are not stored in the legacy file, so discovered documents get distinct generated ones"
	);
	assert_eq!(portfolio(&editor).documents.len(), 2);
	assert!(portfolio(&editor).active_document().is_some());
}

#[tokio::test]
async fn session_state_restores_order_names_and_selection() {
	let mut editor = EditorTestUtils::create();
	new_document(&mut editor, "First");
	let selected = new_document(&mut editor, "Second");
	let state = portfolio(&editor).persisted_state_snapshot();
	let expected: Vec<_> = state.documents.iter().map(|info| (info.id, info.name.clone())).collect();

	restart(&mut editor, state);

	let restored = portfolio(&editor).persisted_state_snapshot();
	assert_eq!(restored.documents.iter().map(|info| (info.id, info.name.clone())).collect::<Vec<_>>(), expected);
	assert_eq!(portfolio(&editor).active_document_id, Some(selected));
}

#[tokio::test]
async fn session_entries_without_stored_content_are_dropped() {
	let mut editor = EditorTestUtils::create();
	let kept = new_document(&mut editor, "Kept");
	let mut state = portfolio(&editor).persisted_state_snapshot();
	let mut ghost = state.documents[0].clone();
	ghost.id = DocumentId(0xdead);
	state.documents.push(ghost);

	restart(&mut editor, state);

	let ids: Vec<_> = portfolio(&editor).persisted_state_snapshot().documents.iter().map(|info| info.id).collect();
	assert_eq!(ids, vec![kept]);
}

#[tokio::test]
async fn legacy_only_container_gets_storage() {
	let mut editor = EditorTestUtils::create();
	let id = DocumentId(0xabc);
	let container = container(&editor, id).await;
	container.write("legacy.graphite", DocumentMessageHandler::default().serialize_document().as_bytes()).await.unwrap();

	restart(&mut editor, PersistedState::default());

	assert!(portfolio(&editor).documents.contains_key(&id));
	assert!(editor.active_document().storage().is_some());
	assert!(container.exists("manifest.json").await);
	assert!(container.exists("legacy.graphite").await);
}

#[tokio::test]
async fn legacy_mode_reads_legacy_only_and_leaves_storage_files_alone() {
	let mut editor = EditorTestUtils::create();
	dispatch(&mut editor, PreferencesMessage::SaveAsGdd { enabled: false });
	let id = new_document(&mut editor, "Legacy");
	let container = container(&editor, id).await;
	assert!(!container.exists("manifest.json").await);
	container.write("manifest.json", b"broken").await.unwrap();

	restart(&mut editor, PersistedState::default());

	assert!(portfolio(&editor).documents.contains_key(&id));
	assert!(editor.active_document().storage().is_none());
	assert_eq!(container.read("manifest.json").await.unwrap().as_slice(), b"broken");
}

#[tokio::test]
async fn storage_only_container_recovers_and_writes_legacy() {
	let mut editor = EditorTestUtils::create();
	let id = new_document(&mut editor, "Storage");
	let container = container(&editor, id).await;
	container.remove("legacy.graphite").await.unwrap();

	restart(&mut editor, PersistedState::default());

	assert!(portfolio(&editor).documents.contains_key(&id));
	assert!(editor.active_document().storage().is_some());
	assert!(container.exists("legacy.graphite").await);
}

#[tokio::test]
async fn legacy_wins_over_stale_storage_and_history_is_kept() {
	let mut editor = EditorTestUtils::create();
	let id = new_document(&mut editor, "Changed");
	let container = container(&editor, id).await;
	let before = editor.active_document().serialize_document();
	let network_before = editor.active_document().document_network().clone();
	editor.draw_rect(0., 0., 100., 100.).await;
	dispatch(&mut editor, PortfolioMessage::AutoSaveDocument { document_id: id });
	container.write("legacy.graphite", before.as_bytes()).await.unwrap();

	restart(&mut editor, PersistedState::default());

	assert_eq!(editor.active_document().document_network(), &network_before);
	let storage = editor.active_document().storage().expect("storage attached");
	assert!(storage.can_undo(), "the stale stored state stays reachable as history");
}

#[tokio::test]
async fn unchanged_content_keeps_cursor_and_redo_across_restart() {
	let mut editor = EditorTestUtils::create();
	let id = new_document(&mut editor, "Undo");
	let network_before = editor.active_document().document_network().clone();
	editor.draw_rect(0., 0., 100., 100.).await;
	dispatch(&mut editor, PortfolioMessage::AutoSaveDocument { document_id: id });

	editor.handle_message(DocumentMessage::Undo).await;
	assert_eq!(editor.active_document().document_network(), &network_before);
	assert!(editor.active_document().storage().unwrap().can_redo());
	assert!(editor.active_document().is_auto_saved(), "the rebuild completion autosaves the cursor's network");

	restart(&mut editor, PersistedState::default());

	assert_eq!(editor.active_document().document_network(), &network_before);
	let storage = editor.active_document().storage().expect("storage attached");
	assert!(storage.can_redo(), "reopening identical content must not stage a new interaction");
	assert!(!storage.can_undo());
}

#[tokio::test]
async fn closing_removes_the_stored_document() {
	let mut editor = EditorTestUtils::create();
	let id = new_document(&mut editor, "Closed");
	assert_eq!(stored_ids(&editor).await, vec![id]);

	dispatch(&mut editor, PortfolioMessage::CloseDocument { document_id: id });

	assert!(stored_ids(&editor).await.is_empty());
	restart(&mut editor, PersistedState::default());
	assert!(portfolio(&editor).persisted_state_snapshot().documents.is_empty());
}

#[tokio::test]
async fn storage_preference_attaches_and_drops_sessions() {
	let mut editor = EditorTestUtils::create();
	dispatch(&mut editor, PreferencesMessage::SaveAsGdd { enabled: false });
	let id = new_document(&mut editor, "Toggle");
	let container = container(&editor, id).await;
	assert!(editor.active_document().storage().is_none());
	assert!(container.exists("legacy.graphite").await);

	dispatch(&mut editor, PreferencesMessage::SaveAsGdd { enabled: true });
	assert!(editor.active_document().storage().is_some());
	assert!(container.exists("manifest.json").await);

	dispatch(&mut editor, PreferencesMessage::SaveAsGdd { enabled: false });
	assert!(editor.active_document().storage().is_none());
}

#[tokio::test]
async fn unreadable_document_is_reported_and_kept_until_discarded() {
	let mut editor = EditorTestUtils::create();
	let id = DocumentId(7);
	container(&editor, id).await.write("legacy.graphite", b"not a document").await.unwrap();

	restart(&mut editor, PersistedState::default());

	assert!(!portfolio(&editor).documents.contains_key(&id));
	assert!(portfolio(&editor).persisted_state_snapshot().documents.is_empty());
	assert_eq!(stored_ids(&editor).await, vec![id]);

	dispatch(&mut editor, FailedDocumentsMessage::DiscardFailedToLoadDocuments);
	assert!(stored_ids(&editor).await.is_empty());
}
