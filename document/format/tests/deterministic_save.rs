// Saving an unchanged document gives the same bytes every time, reopened in between or not.
#![cfg(all(feature = "conversion", feature = "zip", feature = "xz"))]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{ExportFormat, ExportOptions, GddV1, GddV1Layout};
use document_graph_storage::{Implementation, Network, NetworkId, Node, NodeId, PeerId, Priority, RegistryDelta, ResourceId, SourceKey, TimeStamp, UserId, to_value};
use graph_craft::application_io::resource::{HashMapResourceStorage, ResourceStorage};
use graphene_resource::DataSource;
use std::collections::BTreeMap;
use std::path::Path;

const LATEST: TimeStamp = TimeStamp {
	counter: u64::MAX,
	peer: PeerId(u64::MAX),
};

fn document(peer: u64) -> GddV1 {
	GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(peer), UserId(peer), 1, "editor".into(), "stdlib".into()).expect("create")
}

/// Networks and nodes, some of each removed again, plus link-only resources whose bytes sit in the byte store.
fn edits(peer: u64, byte_store: &HashMapResourceStorage) -> Vec<RegistryDelta> {
	let mut ops = Vec::new();
	for index in 0..8 {
		let network = NetworkId(peer * 1000 + index);
		let removed = index % 3 == 0;
		ops.push(RegistryDelta::AddNetwork {
			id: network,
			network: Network::default(),
		});
		for node in 0..8 {
			let id = NodeId(network.0 * 100 + node);
			let node = Node::new(network, Implementation::ProtoNode(ResourceId::new()), 2);
			ops.push(RegistryDelta::AddNode { id, node: node.clone() });
			if removed || id.0 % 3 == 0 {
				ops.push(RegistryDelta::RemoveNode { id, snapshot: node });
			}
		}
		if removed {
			ops.push(RegistryDelta::RemoveNetwork {
				id: network,
				snapshot: Network::default(),
			});
		}

		let id = ResourceId::new();
		let hash = byte_store.store(format!("linked {peer} {index}").as_bytes());
		let key = SourceKey {
			priority: Priority::new(0.).unwrap(),
			peer: PeerId(peer),
		};
		let source = to_value(&DataSource::Font {
			family: format!("Family {index}"),
			style: None,
		})
		.unwrap();
		ops.push(RegistryDelta::AddSource { id, key, source });
		ops.push(RegistryDelta::SetResourceHash { id, hash: Some(hash) });
	}
	ops
}

fn busy_document(byte_store: &HashMapResourceStorage) -> GddV1 {
	let mut gdd = document(1);
	gdd.stage_ops(edits(1, byte_store)).expect("stage");

	for peer in 2..8 {
		let mut other = document(peer);
		for hot_op in other.stage_ops(edits(peer, byte_store)).expect("stage") {
			gdd.apply_hot_op(hot_op).expect("apply");
		}
	}
	gdd.retire(LATEST).expect("retire");

	let mut embedded = Vec::new();
	for index in 0..16 {
		let id = ResourceId::new();
		gdd.add_resource(id, format!("embedded {index}").as_bytes()).expect("add resource");
		embedded.push((id, gdd.registry().resources[&id].clone()));
	}
	let removed: Vec<_> = embedded.into_iter().step_by(3).map(|(id, snapshot)| RegistryDelta::RemoveResource { id, snapshot }).collect();
	gdd.stage_ops(removed).expect("stage");

	// Left hot, so the save also carries a hot log.
	gdd.stage_ops([RegistryDelta::AddNetwork {
		id: NetworkId(99),
		network: Network::default(),
	}])
	.expect("stage");
	gdd
}

fn option_sets() -> [ExportOptions; 3] {
	let editor_save = ExportOptions {
		include_history: false,
		..Default::default()
	};
	let embed_all = ExportOptions {
		embed_all_resources: true,
		..Default::default()
	};
	[ExportOptions::default(), editor_save, embed_all]
}

fn read_folder(root: &Path) -> BTreeMap<String, Vec<u8>> {
	let mut files = BTreeMap::new();
	let mut pending = vec![root.to_path_buf()];
	while let Some(dir) = pending.pop() {
		for entry in std::fs::read_dir(dir).unwrap() {
			let path = entry.unwrap().path();
			if path.is_dir() {
				pending.push(path);
			} else {
				files.insert(path.strip_prefix(root).unwrap().to_string_lossy().into_owned(), std::fs::read(&path).unwrap());
			}
		}
	}
	files
}

async fn reopen(archive: &[u8]) -> GddV1 {
	GddV1::open_from_archive(archive, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.expect("reopen")
}

#[test]
fn saving_an_unchanged_document_twice_writes_the_same_archive() {
	futures::executor::block_on(async {
		let byte_store = HashMapResourceStorage::new();
		let gdd = busy_document(&byte_store);
		let legacy = b"legacy document".as_slice();

		for format in [ExportFormat::Zip, ExportFormat::Xz] {
			for options in option_sets() {
				let first = gdd.export_to_bytes(format, options, &byte_store, Some(legacy)).await.expect("export");
				let second = gdd.export_to_bytes(format, options, &byte_store, Some(legacy)).await.expect("export");
				assert!(first == second, "{format:?} with {options:?} differs between two saves");

				let reopened = reopen(&first).await;
				let third = reopened.export_to_bytes(format, options, &byte_store, Some(legacy)).await.expect("export");
				assert!(first == third, "{format:?} with {options:?} differs after a reopen");

				let reopened_again = reopen(&third).await;
				let fourth = reopened_again.export_to_bytes(format, options, &byte_store, Some(legacy)).await.expect("export");
				assert!(first == fourth, "{format:?} with {options:?} differs after a second reopen");
			}
		}
	});
}

#[test]
fn saving_an_unchanged_document_twice_writes_the_same_folder() {
	futures::executor::block_on(async {
		let byte_store = HashMapResourceStorage::new();
		let gdd = busy_document(&byte_store);
		let dir = tempfile::tempdir().unwrap();

		for (index, options) in option_sets().into_iter().enumerate() {
			let first = dir.path().join(format!("first {index}"));
			let second = dir.path().join(format!("second {index}"));
			let reopened = dir.path().join(format!("reopened {index}"));

			gdd.export(&first, ExportFormat::Folder, options, &byte_store, None).await.expect("export");
			gdd.export(&second, ExportFormat::Folder, options, &byte_store, None).await.expect("export");
			assert!(read_folder(&first) == read_folder(&second), "folder with {options:?} differs between two saves");

			let archive = gdd.export_to_bytes(ExportFormat::Zip, options, &byte_store, None).await.expect("export");
			reopen(&archive).await.export(&reopened, ExportFormat::Folder, options, &byte_store, None).await.expect("export");
			assert!(read_folder(&first) == read_folder(&reopened), "folder with {options:?} differs after a reopen");
		}
	});
}
