// How opening copes with the histories and interruptions real documents have.
#![cfg(feature = "migrations")]

use document_container::backends::memory::MemoryBackend;
use document_container::{AnyContainer, AsyncContainer};
use document_format::{ExportFormat, ExportOptions, GddV1, GddV1Layout, Layout};
use document_graph_storage::{EncodedDeclaration, Implementation, NoMetadata, PeerId, Subject, UserId, rev_attr, user_attr};
use futures::executor::block_on;
use graph_craft::application_io::resource::HashMapResourceStorage;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_resource::ResourceRegistry;
use migration_core::rename;
use migration_formats as _;

rename!("review::FadeNode"@1 => "review::OpacityNode"@2);

const FADE: &str = "review::FadeNode";
const OTHER: &str = "review::OtherNode";

fn network_of(nodes: &[(u64, &'static str)]) -> NodeNetwork {
	let node = |identifier: &'static str| DocumentNode {
		implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
		call_argument: concrete!(()),
		..Default::default()
	};
	NodeNetwork {
		nodes: nodes.iter().map(|(id, identifier)| (NodeId(*id), node(identifier))).collect(),
		..Default::default()
	}
}

fn create() -> GddV1 {
	GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(7), 1, "editor".into(), "std".into()).unwrap()
}

fn commit(gdd: &mut GddV1, nodes: &[(u64, &'static str)], store: &HashMapResourceStorage) {
	gdd.commit_from_runtime(&network_of(nodes), &NoMetadata, &ResourceRegistry::new(), store).unwrap();
}

fn target() -> Implementation {
	Implementation::ProtoNode(EncodedDeclaration::new("review::OpacityNode".to_string(), 2).unwrap().id)
}

fn source() -> Implementation {
	Implementation::ProtoNode(EncodedDeclaration::new(FADE.to_string(), 1).unwrap().id)
}

/// Undoing and then editing is ordinary editor use, and leaves an abandoned branch in history.
#[test]
fn undo_then_edit_still_opens() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		commit(&mut gdd, &[(1, FADE)], &store);
		commit(&mut gdd, &[(1, FADE), (2, FADE)], &store);
		gdd.undo().unwrap();
		commit(&mut gdd, &[(1, FADE), (3, FADE)], &store);
		let (working, layout) = gdd.into_storage();

		let mut migrated = GddV1::open_in(working, layout).await.unwrap();
		let nodes = |gdd: &GddV1| gdd.registry().node_instances.keys().map(|id| id.0).collect::<std::collections::BTreeSet<_>>();
		let edited = nodes(&migrated);
		assert_eq!(edited.len(), 2);
		assert!(migrated.registry().node_instances.values().all(|node| node.implementation() == &target()));

		// History survived the rewrite, so undo reaches back through the edit
		migrated.undo().unwrap();
		assert_eq!(nodes(&migrated).len(), 1);
		assert!(migrated.registry().node_instances.values().all(|node| node.implementation() == &target()));
		migrated.redo().unwrap();
		assert_eq!(nodes(&migrated), edited);
	});
}

/// The folder backend lists and removes files for real, unlike the memory backend the other tests use.
#[test]
fn a_migration_on_disk_leaves_no_staging_behind() {
	block_on(async {
		let directory = tempfile::tempdir().unwrap();
		let path = directory.path().join("document");
		let mut gdd = GddV1::create(&path, PeerId(7), 1, "editor".into(), "std".into()).unwrap();
		commit(&mut gdd, &[(1, FADE)], &HashMapResourceStorage::new());
		drop(gdd);

		let migrated = GddV1::open(&path).await.unwrap();
		assert!(migrated.migrated());
		assert!(!path.join(GddV1Layout.staging_dir()).join("committed").exists());
		drop(migrated);

		let reopened = GddV1::open(&path).await.unwrap();
		assert!(!reopened.migrated());
		let implementations: Vec<_> = reopened.registry().node_instances.values().map(|node| node.implementation().clone()).collect();
		assert_eq!(implementations, [target()]);
	});
}

async fn files(working: &AnyContainer) -> Vec<(String, Vec<u8>)> {
	let mut files = Vec::new();
	for path in working.list("").await.unwrap() {
		let bytes = working.read(&path).await.unwrap();
		files.push((path, bytes.as_slice().to_vec()));
	}
	files
}

/// The document as it was before migrating, with the migrated files staged as `persist_all` left them when it was interrupted.
async fn interrupted_migration(committed: bool, already_moved: &[&str]) -> (AnyContainer, GddV1Layout, Vec<(String, Vec<u8>)>) {
	let store = HashMapResourceStorage::new();
	let mut gdd = create();
	commit(&mut gdd, &[(1, FADE)], &store);
	let (working, layout) = gdd.into_storage();
	let before = files(&working).await;

	let migrated = GddV1::open_in(working.clone(), layout).await.unwrap();
	assert!(migrated.migrated());
	let (working, layout) = migrated.into_storage();
	let after = files(&working).await;

	let staging = layout.staging_dir();
	for (path, bytes) in &after {
		working.write_non_blocking(&format!("{staging}/{path}"), bytes).unwrap();
	}
	if committed {
		working.write_non_blocking(&format!("{staging}/committed"), &[]).unwrap();
	}
	for (path, bytes) in &before {
		if !already_moved.iter().any(|moved| path.contains(moved)) {
			working.write_non_blocking(path, bytes).unwrap();
		}
	}
	(working, layout, after)
}

async fn assert_settled(working: AnyContainer, layout: GddV1Layout) {
	let store = HashMapResourceStorage::new();
	let mut reopened = GddV1::open_in(working.clone(), layout).await.unwrap();
	let implementations: Vec<_> = reopened.registry().node_instances.values().map(|node| node.implementation().clone()).collect();
	assert_eq!(implementations, [target()]);
	assert!(working.list(layout.staging_dir()).await.unwrap().is_empty(), "the staging folder should be gone");
	reopened
		.commit_from_runtime(&network_of(&[(1, "review::OpacityNode"), (2, OTHER)]), &NoMetadata, &ResourceRegistry::new(), &store)
		.unwrap();
}

/// Staging never finished, so the document is still its old self and migrates again.
#[test]
fn an_uncommitted_rewrite_is_discarded() {
	block_on(async {
		let (working, layout, _) = interrupted_migration(false, &[]).await;
		assert_settled(working, layout).await;
	});
}

/// The marker made it, so the reopen completes the rewrite instead of trusting a mix of old and new files.
#[test]
fn a_committed_rewrite_finishes() {
	block_on(async {
		for already_moved in [&[][..], &["history"][..], &["history", "registry"][..], &["history", "registry", "session"][..]] {
			let (working, layout, after) = interrupted_migration(true, already_moved).await;
			let reopened = GddV1::open_in(working.clone(), layout).await.unwrap();
			assert!(!reopened.migrated(), "finishing the staged rewrite leaves nothing to migrate");
			drop(reopened);

			for (path, bytes) in &after {
				assert_eq!(working.read(path).await.unwrap().as_slice(), bytes.as_slice(), "{path}");
			}
			assert_settled(working, layout).await;
		}
	});
}

/// What a working copy holds once a history-less `.gdd` opened and was edited: the full registry, and only the new deltas.
fn keep_deltas_from(gdd: &GddV1, first: usize) -> Vec<u8> {
	let codec = gdd.manifest().codecs.history;
	let mut buffer = Vec::new();
	for delta in gdd.session().history().skip(first) {
		codec.append(&mut buffer, delta).unwrap();
	}
	buffer
}

async fn with_partial_history(later_edit: &[(u64, &'static str)]) -> Result<GddV1, document_format::Error> {
	let store = HashMapResourceStorage::new();
	let mut gdd = create();
	commit(&mut gdd, &[(1, FADE)], &store);
	let bootstrap_length = gdd.session().history_len();
	commit(&mut gdd, later_edit, &store);

	let history = keep_deltas_from(&gdd, bootstrap_length);
	let path = document_format::io::path_for(GddV1Layout.history_basename(), gdd.manifest().codecs.history);
	let (working, layout) = gdd.into_storage();
	working.write_non_blocking(&path, &history).unwrap();

	GddV1::open_in(working, layout).await
}

/// The node in the registry never appears in the history that survives, so nothing looks pending.
#[test]
fn a_source_node_only_in_the_snapshot_still_migrates() {
	block_on(async {
		let opened = with_partial_history(&[(1, FADE), (2, OTHER)]).await.unwrap();
		let node = opened
			.registry()
			.node_instances
			.values()
			.find(|node| node.implementation() == &source() || node.implementation() == &target())
			.unwrap();
		assert_eq!(node.implementation(), &target(), "the document was edited after it was saved, and its old node is never migrated");
	});
}

/// The new edit does reference the source, so the rewrite runs and finds a parent it can't resolve.
#[test]
fn a_partial_history_referencing_the_source_still_opens() {
	block_on(async {
		let opened = with_partial_history(&[(1, FADE), (2, FADE)]).await.unwrap();
		assert!(opened.registry().node_instances.values().all(|node| node.implementation() == &target()));
		assert_eq!(opened.registry().node_instances.len(), 2);
	});
}

/// This is what the partial histories above stand in for: editing a `.gdd` that was saved without history.
#[test]
fn editing_a_history_less_gdd_persists_a_whole_history() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		commit(&mut gdd, &[(1, OTHER)], &store);
		let options = ExportOptions {
			include_history: false,
			..Default::default()
		};
		let archive = gdd.export_to_bytes(ExportFormat::Zip, options, &store, None).await.unwrap();

		let mut opened = GddV1::open_from_archive(&archive, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.unwrap();
		assert!(!opened.migrated());
		commit(&mut opened, &[(1, OTHER), (2, OTHER)], &store);
		let (working, layout) = opened.into_storage();

		let reopened = GddV1::open_in(working, layout).await.unwrap();
		let first = reopened.session().history().next().unwrap();
		assert_eq!(first.parent, None, "the stored history starts mid-air: {:?}", first.kind);
	});
}

/// A swap of a node's implementation is the only delta naming the new declaration, and redo needs it.
#[test]
fn reopening_with_an_undone_implementation_swap_can_redo() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		commit(&mut gdd, &[(1, OTHER)], &store);
		commit(&mut gdd, &[(1, OTHER), (2, OTHER)], &store);
		commit(&mut gdd, &[(1, "review::SwappedNode"), (2, OTHER)], &store);
		gdd.undo().unwrap();
		let (working, layout) = gdd.into_storage();

		let mut reopened = GddV1::open_in(working, layout).await.unwrap();
		let declarations = reopened.declarations(&store).await;
		reopened.redo().unwrap();
		let converted = reopened.registry().to_runtime_with_full_metadata(&declarations);
		assert!(converted.is_ok(), "{}", converted.err().unwrap());
	});
}

/// Checks of what's expected to work, so a pass here means something.
#[test]
fn hot_ops_migrate_once_and_counters_survive() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		commit(&mut gdd, &[(1, FADE)], &store);
		gdd.stage_runtime_snapshot(&network_of(&[(1, FADE), (2, FADE)]), &NoMetadata, &ResourceRegistry::new(), &store).unwrap();
		let (hot, clock, sequence, marks) = (
			gdd.session().hot_log().len(),
			gdd.session().clock_counter(),
			gdd.session().last_hot_sequence(),
			gdd.session().retired_marks().clone(),
		);
		assert!(hot > 0);
		let (working, layout) = gdd.into_storage();

		let migrated = GddV1::open_in(working, layout).await.unwrap();
		assert!(migrated.session().hot_log().is_empty());
		assert_eq!(migrated.registry().node_instances.values().filter(|node| node.implementation() == &target()).count(), 2);
		assert!(migrated.session().clock_counter() >= clock);
		assert_eq!(migrated.session().last_hot_sequence(), sequence);
		assert_ne!(migrated.session().retired_marks(), &marks);
		let registry = migrated.registry().clone();
		let (working, layout) = migrated.into_storage();

		let reopened = GddV1::open_in(working, layout).await.unwrap();
		assert!(!reopened.migrated());
		assert!(reopened.session().hot_log().is_empty());
		assert!(reopened.registry().value_equal(&registry));
	});
}

#[test]
fn the_published_frontier_follows_the_rewrite() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		commit(&mut gdd, &[(1, FADE)], &store);
		commit(&mut gdd, &[(1, FADE), (2, FADE)], &store);
		let head = gdd.session().head_rev().unwrap();
		gdd.publish_up_to(head).unwrap();
		assert!(!gdd.can_undo());
		let (working, layout) = gdd.into_storage();

		let migrated = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(migrated.session().last_broadcast_rev(), migrated.session().head_rev());
		assert!(!migrated.can_undo());
	});
}

/// History only, as `include_registry: false` exports it, under a truncating format step.
#[test]
fn registry_only_export_opens_and_migrates() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		commit(&mut gdd, &[(1, FADE)], &store);
		let options = ExportOptions {
			include_history: false,
			..Default::default()
		};
		let bytes = gdd.export_to_bytes(ExportFormat::Zip, options, &store, None).await.unwrap();

		let opened = GddV1::open_from_archive(&bytes, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.unwrap();
		assert!(opened.migrated());
		let implementations: Vec<_> = opened.registry().node_instances.values().map(|node| node.implementation().clone()).collect();
		assert_eq!(implementations, [target()]);
	});
}

/// A migration changes every rev, and a label stated about one has to follow it.
#[test]
fn a_label_on_a_step_follows_the_rewrite() {
	block_on(async {
		let mut gdd = create();
		commit(&mut gdd, &[(1, FADE)], &HashMapResourceStorage::new());
		let head = gdd.session().head_rev().unwrap();
		gdd.record(Subject::Rev(head), rev_attr::LABEL, "first version".into(), 1.).unwrap();
		gdd.record(Subject::User(UserId(7)), user_attr::NAME, "someone".into(), 1.).unwrap();
		let (working, layout) = gdd.into_storage();

		let migrated = GddV1::open_in(working, layout).await.unwrap();
		let new_head = migrated.session().head_rev().unwrap();
		assert_ne!(new_head, head);
		assert_eq!(migrated.metadata().rev_label(new_head), Some("first version"));
		assert_eq!(migrated.metadata().user_name(UserId(7)), Some("someone"));

		let (working, layout) = migrated.into_storage();
		let reopened = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(reopened.metadata().rev_label(new_head), Some("first version"), "the remapped fact is what's stored");
	});
}

/// Truncating leaves no rev for a label to follow, but facts about users have nothing to do with history.
#[test]
fn truncating_history_keeps_only_the_facts_about_users() {
	block_on(async {
		let store = HashMapResourceStorage::new();
		let mut gdd = create();
		commit(&mut gdd, &[(1, FADE)], &store);
		let bootstrap_length = gdd.session().history_len();
		commit(&mut gdd, &[(1, FADE), (2, FADE)], &store);
		let head = gdd.session().head_rev().unwrap();
		gdd.record(Subject::Rev(head), rev_attr::LABEL, "second version".into(), 1.).unwrap();
		gdd.record(Subject::User(UserId(7)), user_attr::NAME, "someone".into(), 1.).unwrap();

		let history = keep_deltas_from(&gdd, bootstrap_length);
		let path = document_format::io::path_for(GddV1Layout.history_basename(), gdd.manifest().codecs.history);
		let (working, layout) = gdd.into_storage();
		working.write_non_blocking(&path, &history).unwrap();

		let opened = GddV1::open_in(working, layout).await.unwrap();
		assert_eq!(opened.metadata().rev_label(head), None);
		assert!(!opened.metadata().facts().any(|fact| matches!(fact.subject, Subject::Rev(_))));
		assert_eq!(opened.metadata().user_name(UserId(7)), Some("someone"));
	});
}
