use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{Codec, Error, Gdd, GddV1, Layout, Manifest, io, manifest};
use graph_storage::{HotOp, Network, NetworkId, PeerId, ROOT_NETWORK, RegistryDelta, TimeStamp};

fn empty_container() -> AnyContainer {
	AnyContainer::Memory(MemoryBackend::new())
}

/// A resource byte store for export calls. Empty unless a test pre-populates it; only consulted when
/// `embed_all_resources` is set.
fn empty_byte_store() -> graph_craft::application_io::resource::HashMapResourceStorage {
	graph_craft::application_io::resource::HashMapResourceStorage::new()
}

/// A one-node network referencing `id` via a `TaggedValue::Resource` input. Conversion only snapshots
/// resources the network references, so a resource needs a referencing node to survive into storage.
fn network_referencing_resource(id: graphene_resource::ResourceId) -> graph_craft::document::NodeNetwork {
	use graph_craft::ProtoNodeIdentifier;
	use graph_craft::document::value::TaggedValue;
	use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};

	NodeNetwork {
		nodes: [(
			NodeId(0),
			DocumentNode {
				inputs: vec![NodeInput::value(TaggedValue::Resource(id), false)],
				implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
		..Default::default()
	}
}

#[test]
fn create_in_round_trips_empty_document() {
	futures::executor::block_on(async {
		let container = empty_container();

		let created = match Gdd::<GddV1>::create_in(container, GddV1, PeerId(7), 0xFEED, "editor-x".into(), "stdlib-x".into()).await {
			Ok(gdd) => gdd,
			Err(error) => panic!("create_in failed: {error:?}"),
		};

		let (working, layout) = created.into_storage();
		let reopened = match Gdd::<GddV1>::open_in(working, layout).await {
			Ok(gdd) => gdd,
			Err(error) => panic!("open_in failed: {error:?}"),
		};

		assert_eq!(reopened.session().peer(), PeerId(7));
		assert!(reopened.registry().node_instances.is_empty());
		assert!(reopened.registry().networks.is_empty());
	});
}

#[test]
fn open_in_rejects_wrong_format_magic() {
	futures::executor::block_on(async {
		let container = empty_container();
		let layout = GddV1;

		let mut bogus = Manifest::new(0xC0DE, PeerId(1), "ed".into(), "std".into());
		bogus.format = "not-gdd".into();
		io::write_single(&container, layout.manifest_basename(), Codec::Json, &bogus).unwrap();

		match Gdd::<GddV1>::open_in(container, layout).await {
			Err(Error::WrongFormat { .. }) => {}
			Ok(_) => panic!("expected WrongFormat, got Ok"),
			Err(other) => panic!("expected WrongFormat, got {other:?}"),
		}
	});
}

#[test]
fn manifest_returns_what_create_in_wrote() {
	futures::executor::block_on(async {
		let gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(13), 0xC0FFEE, "ed-1.2".into(), "std-0.7".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let manifest = gdd.manifest();
		assert_eq!(manifest.peer_id, PeerId(13));
		assert_eq!(manifest.document_uuid, 0xC0FFEE);
		assert_eq!(manifest.editor_version, "ed-1.2");
		assert_eq!(manifest.stdlib_version, "std-0.7");
		assert_eq!(manifest.format, manifest::FORMAT_MAGIC);
	});
}

#[test]
fn update_manifest_changes_visible_after_reopen() {
	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(1), 0xAB, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		gdd.update_manifest(|m| m.editor_version = "ed-NEW".into())
			.unwrap_or_else(|error| panic!("update_manifest failed: {error:?}"));

		let (working, layout) = gdd.into_storage();
		let reopened = Gdd::<GddV1>::open_in(working, layout).await.unwrap_or_else(|error| panic!("open_in failed: {error:?}"));
		let manifest = reopened.manifest();
		assert_eq!(manifest.editor_version, "ed-NEW");
	});
}

#[test]
fn apply_hot_op_persists_to_hot_log_and_survives_reopen() {
	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(5), 0xDEAD, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		// AddNetwork on the root network. Idempotent at apply, so two hot ops applied in sequence
		// produces one network in the registry.
		let hot_op = HotOp {
			op: RegistryDelta::AddNetwork {
				id: ROOT_NETWORK,
				network: Network::default(),
			},
			timestamp: TimeStamp { counter: 1, peer: PeerId(5) },
		};
		gdd.apply_hot_op(hot_op).unwrap_or_else(|error| panic!("apply_hot_op failed: {error:?}"));

		assert!(gdd.registry().networks.contains_key(&ROOT_NETWORK), "hot op should have created the root network in memory");

		let (working, layout) = gdd.into_storage();
		let reopened = Gdd::<GddV1>::open_in(working, layout).await.unwrap_or_else(|error| panic!("open_in failed: {error:?}"));

		assert!(reopened.registry().networks.contains_key(&ROOT_NETWORK), "hot op should have been replayed from the hot log on reopen");
	});
}

#[test]
fn retire_moves_eligible_hot_ops_to_history_and_keeps_rest() {
	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(5), 0xDEAD, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		// Two hot ops: one with low timestamp (will retire), one with high (will stay).
		let early = HotOp {
			op: RegistryDelta::AddNetwork {
				id: ROOT_NETWORK,
				network: Network::default(),
			},
			timestamp: TimeStamp { counter: 1, peer: PeerId(5) },
		};
		let late = HotOp {
			op: RegistryDelta::AddNetwork {
				id: NetworkId(42),
				network: Network::default(),
			},
			timestamp: TimeStamp { counter: 10, peer: PeerId(5) },
		};
		gdd.apply_hot_op(early).unwrap();
		gdd.apply_hot_op(late).unwrap();
		assert_eq!(gdd.session().hot_log().len(), 2);

		// Retire only up to timestamp 5 → drains the early op, leaves the late one.
		let cutoff = TimeStamp { counter: 5, peer: PeerId(5) };
		gdd.retire(cutoff).unwrap_or_else(|error| panic!("retire failed: {error:?}"));

		assert_eq!(gdd.session().hot_log().len(), 1, "late hot op should still be in hot log");
		assert_eq!(gdd.session().history().count(), 1, "early hot op should be in retired history");

		// Reopen and confirm survival: hot log has the late op (replayed), history has the early op.
		let (working, layout) = gdd.into_storage();
		let reopened = Gdd::<GddV1>::open_in(working, layout).await.unwrap_or_else(|error| panic!("open_in failed: {error:?}"));

		assert!(reopened.registry().networks.contains_key(&ROOT_NETWORK), "retired op's effect should be in registry");
		assert!(reopened.registry().networks.contains_key(&NetworkId(42)), "hot op's effect should be replayed");
		assert_eq!(reopened.session().history().count(), 1);
		assert_eq!(reopened.session().hot_log().len(), 1);

		// Manifest bumped.
		assert!(reopened.manifest().last_retired_at.is_some(), "retire should bump last_retired_at");
	});
}

#[test]
fn export_folder_round_trips_through_open() {
	use document_format::{ExportFormat, ExportOptions};

	futures::executor::block_on(async {
		let gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(3), 0xAB, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let dir = tempfile::tempdir().unwrap();
		let dest = dir.path().join("export");

		gdd.export(&dest, ExportFormat::Folder, ExportOptions::default(), &empty_byte_store())
			.await
			.unwrap_or_else(|error| panic!("export failed: {error:?}"));

		// Payloads keep the working-copy codecs: registry is MessagePack (`.bin`), manifest is JSON.
		assert!(dest.join("registry.bin").exists());
		assert!(dest.join("manifest.json").exists());
		assert!(dest.join("session.json").exists());
		assert!(!dest.join("hot-log.bin").exists());
		assert!(!dest.join("hot-log.frames").exists());

		// And the export is itself openable.
		let reopened = Gdd::<GddV1>::open(&dest).await.unwrap_or_else(|error| panic!("open failed: {error:?}"));
		assert_eq!(reopened.session().peer(), PeerId(3));
	});
}

#[test]
fn export_zip_round_trips_via_deserialize() {
	use document_container::archive::{Archive, Zip};
	use document_format::{ExportFormat, ExportOptions};

	futures::executor::block_on(async {
		let gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(4), 0xCD, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let dir = tempfile::tempdir().unwrap();
		let dest = dir.path().join("doc.gdd.zip");

		gdd.export(&dest, ExportFormat::Zip, ExportOptions::default(), &empty_byte_store())
			.await
			.unwrap_or_else(|error| panic!("export failed: {error:?}"));

		let bytes = std::fs::read(&dest).unwrap();
		let mut restored = document_container::backends::memory::MemoryBackend::new();
		Zip::open(std::io::Cursor::new(&bytes), &mut restored).unwrap();
		use document_container::Container;
		assert!(restored.exists("manifest.json"));
		assert!(restored.exists("registry.bin"));
		assert!(restored.exists("session.json"));
		assert!(!restored.exists("hot-log.frames"));
	});
}

#[test]
fn export_rejects_invalid_options() {
	use document_format::{ExportFormat, ExportOptions};

	futures::executor::block_on(async {
		let gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(1), 0xEF, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let dir = tempfile::tempdir().unwrap();
		let dest = dir.path().join("nope");
		let options = ExportOptions {
			include_registry: false,
			include_history: false,
			embed_all_resources: false,
		};

		match gdd.export(&dest, ExportFormat::Folder, options, &empty_byte_store()).await {
			Err(Error::InvalidExportOptions(_)) => {}
			Ok(_) => panic!("expected InvalidOptions, got Ok"),
			Err(other) => panic!("expected InvalidOptions, got {other:?}"),
		}
	});
}

#[test]
fn resource_round_trip_add_read_remove() {
	use graphene_resource::{ResourceHash, ResourceId};

	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(99), 0xCAFE, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let payload = b"deadbeef cafe babe";
		let hash = ResourceHash::from(&payload[..]);
		let id = ResourceId::new();

		assert!(!gdd.has_resource(&hash).await);
		gdd.add_resource(id, payload).unwrap_or_else(|error| panic!("add_resource failed: {error:?}"));
		assert!(gdd.has_resource(&hash).await);

		let read_back = gdd.read_resource(&hash).await.unwrap();
		assert_eq!(read_back.as_slice(), payload);

		// The registry records the resource (entry keyed by id, resolved to the content hash).
		let entry = gdd.registry().resources.get(&id).expect("registry records the added resource");
		assert_eq!(entry.hash, Some(hash));

		let hashes = gdd.resource_hashes().await.unwrap();
		assert_eq!(hashes, vec![hash]);

		gdd.remove_resource(&hash).unwrap();
		assert!(!gdd.has_resource(&hash).await);
	});
}

#[test]
fn resource_survives_reopen() {
	use graphene_resource::{ResourceHash, ResourceId};

	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(7), 0xC0DE, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let payload = b"persistent bytes";
		let hash = ResourceHash::from(&payload[..]);
		let id = ResourceId::new();
		gdd.add_resource(id, payload).unwrap();

		let (working, layout) = gdd.into_storage();
		let reopened = Gdd::<GddV1>::open_in(working, layout).await.unwrap_or_else(|error| panic!("open_in failed: {error:?}"));

		assert!(reopened.has_resource(&hash).await);
		assert_eq!(reopened.read_resource(&hash).await.unwrap().as_slice(), payload);

		// The registry entry replicated through the history file and survives reopen.
		let entry = reopened.registry().resources.get(&id).expect("reopened registry records the resource");
		assert_eq!(entry.hash, Some(hash));
	});
}

#[test]
fn resource_from_path_uses_fs_copy_on_folder_backend() {
	use document_container::AnyContainer;
	use document_container::backends::folder::FolderBackend;
	use graphene_resource::{ResourceHash, ResourceId};

	futures::executor::block_on(async {
		// Need a folder-backed working copy to exercise the fs::copy path.
		let working_dir = tempfile::tempdir().unwrap();
		let working = AnyContainer::Folder(FolderBackend::create(working_dir.path()).unwrap());
		let mut gdd = Gdd::<GddV1>::create_in(working, GddV1, PeerId(1), 0xAB, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		// Source file outside the working copy.
		let payload = b"external resource bytes";
		let src_dir = tempfile::tempdir().unwrap();
		let src_path = src_dir.path().join("blob");
		std::fs::write(&src_path, payload).unwrap();

		let hash = ResourceHash::from(&payload[..]);
		let id = ResourceId::new();
		gdd.add_resource_from_path(id, hash, &src_path)
			.unwrap_or_else(|error| panic!("add_resource_from_path failed: {error:?}"));

		assert!(gdd.has_resource(&hash).await);
		assert_eq!(gdd.read_resource(&hash).await.unwrap().as_slice(), payload);
	});
}

#[test]
fn export_carries_resources() {
	use document_format::{ExportFormat, ExportOptions};
	use graphene_resource::{ResourceHash, ResourceId};

	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(2), 0xBC, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let payload = b"exported resource";
		let hash = ResourceHash::from(&payload[..]);
		let id = ResourceId::new();
		gdd.add_resource(id, payload).unwrap();

		let dir = tempfile::tempdir().unwrap();
		let dest = dir.path().join("export");
		gdd.export(&dest, ExportFormat::Folder, ExportOptions::default(), &empty_byte_store()).await.unwrap();

		let resource_file = dest.join("resources").join(format!("{hash}"));
		assert!(resource_file.exists(), "exported resource file should exist at {resource_file:?}");
		assert_eq!(std::fs::read(&resource_file).unwrap(), payload);
	});
}

/// `embed_all_resources` makes a link-only resource self-contained: the bytes (which live only in
/// the byte store, not the working copy) are written into the export, the exported registry's chain
/// gains a leading `Embedded` source ahead of the original `Url`, and the export reopens with both.
#[test]
fn embed_all_resources_materializes_link_only_resource() {
	use document_format::{ExportFormat, ExportOptions};
	use graph_craft::application_io::resource::ResourceStorage;
	use graph_storage::NoMetadata;
	use graphene_resource::{DataSource, ResourceHash, ResourceId, ResourceRegistry};

	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(8), 0xF00D, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		// A resource whose only source is a URL, resolved to a hash. The bytes live solely in the
		// byte store; the working copy never holds them.
		let payload = b"bytes behind a url";
		let hash = ResourceHash::from(&payload[..]);
		let byte_store = empty_byte_store();
		byte_store.store(payload);

		let mut resources = ResourceRegistry::new();
		let id = ResourceId::new();
		resources.push_source_back(&id, DataSource::Url("https://example.com/r.bin".parse().unwrap()));
		resources.resolve(&id, hash);

		gdd.commit_from_runtime(&network_referencing_resource(id), &NoMetadata, &resources, &byte_store)
			.unwrap_or_else(|error| panic!("commit_from_runtime failed: {error:?}"));

		// The working copy holds no resource bytes (URL source, nothing embedded yet).
		assert!(!gdd.has_resource(&hash).await);

		let dir = tempfile::tempdir().unwrap();
		let dest = dir.path().join("embedded");
		gdd.export(
			&dest,
			ExportFormat::Folder,
			ExportOptions {
				embed_all_resources: true,
				..Default::default()
			},
			&byte_store,
		)
		.await
		.unwrap_or_else(|error| panic!("export failed: {error:?}"));

		// Bytes materialized into the export.
		let resource_file = dest.join("resources").join(format!("{hash}"));
		assert!(resource_file.exists(), "embedded resource bytes should be written to {resource_file:?}");
		assert_eq!(std::fs::read(&resource_file).unwrap(), payload);

		// Reopen the export: the registry chain now leads with Embedded, keeping the URL as fallback,
		// and the bytes are resolvable from the export itself with no byte store.
		let reopened = Gdd::<GddV1>::open(&dest).await.unwrap_or_else(|error| panic!("open export failed: {error:?}"));
		assert!(reopened.has_resource(&hash).await, "embedded bytes should be resolvable from the export");

		let entry = reopened.registry().resources.get(&id).expect("resource entry survived export");
		assert_eq!(entry.hash, Some(hash));
		let embedded = serde_json::to_value(DataSource::Embedded).unwrap();
		let url = serde_json::to_value(DataSource::Url("https://example.com/r.bin".parse().unwrap())).unwrap();
		let chain: Vec<_> = entry.sources.iter().map(|(_, value)| value.source.clone()).collect();
		assert_eq!(chain, vec![embedded, url], "Embedded leads the chain, URL kept as fallback");
	});
}

/// A plain export (no `embed_all_resources`) still materializes the bytes of an already-`Embedded`
/// resource, pulling from the byte store when the working copy doesn't hold them (the editor case
/// where bytes live in the app-global cache, not the per-document working copy).
#[test]
fn export_materializes_embedded_resource_from_byte_store() {
	use document_format::{ExportFormat, ExportOptions};
	use graph_craft::application_io::resource::ResourceStorage;
	use graph_storage::NoMetadata;
	use graphene_resource::{DataSource, ResourceHash, ResourceId, ResourceRegistry};

	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(9), 0xBEEF, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		// An Embedded resource whose bytes live only in the byte store, not the working copy.
		let payload = b"embedded bytes in the cache";
		let hash = ResourceHash::from(&payload[..]);
		let byte_store = empty_byte_store();
		byte_store.store(payload);

		let mut resources = ResourceRegistry::new();
		let id = ResourceId::new();
		resources.push_source_back(&id, DataSource::Embedded);
		resources.resolve(&id, hash);
		gdd.commit_from_runtime(&network_referencing_resource(id), &NoMetadata, &resources, &byte_store)
			.unwrap_or_else(|error| panic!("commit_from_runtime failed: {error:?}"));

		assert!(!gdd.has_resource(&hash).await, "bytes should not be in the working copy");

		let dir = tempfile::tempdir().unwrap();
		let dest = dir.path().join("plain");
		// Default options: embed_all_resources is false.
		gdd.export(&dest, ExportFormat::Folder, ExportOptions::default(), &byte_store)
			.await
			.unwrap_or_else(|error| panic!("export failed: {error:?}"));

		let resource_file = dest.join("resources").join(format!("{hash}"));
		assert!(resource_file.exists(), "embedded resource bytes should be pulled from the store into {resource_file:?}");
		assert_eq!(std::fs::read(&resource_file).unwrap(), payload);
	});
}

#[test]
fn open_in_rejects_future_format_version() {
	futures::executor::block_on(async {
		let container = empty_container();
		let layout = GddV1;

		let mut future_version = Manifest::new(0xC0DE, PeerId(1), "ed".into(), "std".into());
		future_version.format_version = manifest::SUPPORTED_FORMAT_VERSION + 1;
		io::write_single(&container, layout.manifest_basename(), Codec::Json, &future_version).unwrap();

		match Gdd::<GddV1>::open_in(container, layout).await {
			Err(Error::UnsupportedVersion { .. }) => {}
			Ok(_) => panic!("expected UnsupportedVersion, got Ok"),
			Err(other) => panic!("expected UnsupportedVersion, got {other:?}"),
		}
	});
}

#[test]
fn create_in_records_default_codecs_in_manifest() {
	futures::executor::block_on(async {
		let gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(1), 0xAB, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let codecs = gdd.manifest().codecs;
		assert_eq!(codecs.registry, Codec::MessagePack);
		assert_eq!(codecs.history, Codec::MessagePackFrames);
		assert_eq!(codecs.hot_log, Codec::MessagePackFrames);
		assert_eq!(codecs.session, Codec::Json);
	});
}

/// The `RegisterPeer` op auto-emitted on the first commit rides the hot-op pipeline through
/// persistence and retirement, so the `peer_users` mapping survives a reopen.
#[test]
fn first_commit_registers_peer_and_survives_reopen() {
	use graph_craft::application_io::resource::HashMapResourceStorage;
	use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
	use graph_craft::{ProtoNodeIdentifier, concrete};
	use graph_storage::{NoMetadata, UserId};
	use graphene_resource::ResourceRegistry;

	futures::executor::block_on(async {
		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(21), 0xAB, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let network = NodeNetwork {
			exports: vec![NodeInput::node(core_types::uuid::NodeId(0), 0)],
			nodes: [(
				core_types::uuid::NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(u32), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::identity::IdentityNode")),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};

		gdd.commit_from_runtime(&network, &NoMetadata, &ResourceRegistry::new(), &HashMapResourceStorage::new())
			.unwrap_or_else(|error| panic!("commit_from_runtime failed: {error:?}"));
		assert_eq!(gdd.registry().peer_users.get(&PeerId(21)), Some(&UserId(21)), "first commit registers the peer");

		let (working, layout) = gdd.into_storage();
		let reopened = Gdd::<GddV1>::open_in(working, layout).await.unwrap_or_else(|error| panic!("open_in failed: {error:?}"));
		assert_eq!(reopened.registry().peer_users.get(&PeerId(21)), Some(&UserId(21)), "registration survives reopen");
	});
}

#[test]
fn persist_path_writes_at_manifest_declared_codec_paths() {
	// The manifest declares the on-disk codec for each payload; the persist path must write at the
	// extension that codec implies, and reopen (which reads the codec from the manifest) must find them.
	futures::executor::block_on(async {
		use document_container::AsyncContainer;

		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(5), 0xDEAD, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		let hot_op = HotOp {
			op: RegistryDelta::AddNetwork {
				id: ROOT_NETWORK,
				network: Network::default(),
			},
			timestamp: TimeStamp { counter: 1, peer: PeerId(5) },
		};
		gdd.apply_hot_op(hot_op).unwrap_or_else(|error| panic!("apply_hot_op failed: {error:?}"));

		let (working, layout) = gdd.into_storage();
		// Defaults: hot log is MessagePackFrames (.frames), manifest is always JSON.
		assert!(working.exists(&io::path_for(layout.hot_log_basename(), Codec::MessagePackFrames)).await);
		assert!(working.exists(&io::path_for(layout.manifest_basename(), Codec::Json)).await);

		let reopened = Gdd::<GddV1>::open_in(working, layout).await.unwrap_or_else(|error| panic!("open_in failed: {error:?}"));
		assert!(reopened.registry().networks.contains_key(&ROOT_NETWORK));
	});
}

/// Complete declaration round-trip through the byte store: committing a runtime network with a
/// proto-node persists its `ProtoNode` content into a `ResourceStorage`, and resolving declarations
/// back through that store reconstructs the proto-node identifier in `to_runtime`. This is the
/// editor-shaped path (declaration bytes live in the resource store, not the Gdd container).
#[test]
fn declarations_round_trip_through_byte_store() {
	use graph_craft::application_io::resource::HashMapResourceStorage;
	use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
	use graph_craft::{ProtoNodeIdentifier, concrete};
	use graph_storage::NoMetadata;
	use graphene_resource::ResourceRegistry;

	const PROTO: &str = "graphene_core::ops::identity::IdentityNode";

	futures::executor::block_on(async {
		let network = NodeNetwork {
			exports: vec![NodeInput::node(core_types::uuid::NodeId(0), 0)],
			nodes: [(
				core_types::uuid::NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::import(concrete!(u32), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new(PROTO)),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};

		let mut gdd = Gdd::<GddV1>::create_in(empty_container(), GddV1, PeerId(1), 0xAB, "ed".into(), "std".into())
			.await
			.unwrap_or_else(|error| panic!("create_in failed: {error:?}"));

		// Commit: declaration bytes flow into the byte store, not the Gdd container.
		let byte_store = HashMapResourceStorage::new();
		gdd.commit_from_runtime(&network, &NoMetadata, &ResourceRegistry::new(), &byte_store)
			.unwrap_or_else(|error| panic!("commit_from_runtime failed: {error:?}"));

		// Resolve declarations back through the store and convert to a runtime network.
		let declarations = gdd.declarations(&byte_store).await;
		assert_eq!(declarations.len(), 1, "expected one proto-node declaration resolved from the byte store");

		let (converted, _entries) = gdd.registry().to_runtime_with_metadata(&declarations).unwrap_or_else(|error| panic!("to_runtime failed: {error:?}"));

		let node = converted.nodes.values().next().expect("converted network has the node");
		match &node.implementation {
			DocumentNodeImplementation::ProtoNode(identifier) => assert_eq!(identifier.as_str(), PROTO, "proto-node identifier survived the byte-store round-trip"),
			other => panic!("expected a ProtoNode implementation, got {other:?}"),
		}
	});
}
