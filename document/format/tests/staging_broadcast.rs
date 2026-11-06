#![cfg(feature = "network")]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{HotOp, Network, NetworkId, PeerId, RegistryDelta, ResourceId, Session, UserId};
use graphene_resource::ResourceHash;
use peer_transport::mock::MockNetwork;
use peer_transport::{Event, SyncTarget};

fn document(peer: u64) -> GddV1 {
	GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(peer), UserId(peer), 1, "editor".into(), "stdlib".into()).expect("create")
}

fn settle(network: &mut MockNetwork, peers: &mut [&mut GddV1]) {
	for _ in 0..16 {
		for peer in peers.iter_mut() {
			peer.poll_peers();
		}
		network.deliver_all();
	}
}

#[cfg(feature = "conversion")]
#[test]
fn staging_a_constructed_batch_reaches_the_room() {
	let mut host = document(1);
	let mut network = MockNetwork::new(1);
	let host_endpoint = network.endpoint();
	let guest_endpoint = network.endpoint();
	let (host_id, guest_id) = (host_endpoint.id(), guest_endpoint.id());
	host.share(host_endpoint, UserId(1)).expect("share");
	network.connect(host_id);
	network.connect(guest_id);

	// Get the host's greeting out of the way.
	host.poll_peers();
	network.deliver_all();
	assert_eq!(network.pending(), 0);

	let byte_store = graph_craft::application_io::resource::HashMapResourceStorage::new();
	let staged = host
		.stage_constructed_ops(
			vec![RegistryDelta::AddNetwork {
				id: NetworkId(9),
				network: Network::default(),
			}],
			&Default::default(),
			&byte_store,
		)
		.expect("stage");

	assert!(!staged.is_empty(), "the staged hot ops come back for the caller");
	assert!(network.pending() > 0, "a staged batch must be broadcast to the room");
}

/// A guest leaves retirement to the host, so a resource it adds reaches the room as a hot op.
#[test]
fn a_resource_a_guest_adds_reaches_the_host() {
	let mut host = document(1);
	let mut guest = document(2);
	let mut network = MockNetwork::new(1);
	let host_endpoint = network.endpoint();
	let guest_endpoint = network.endpoint();
	let (host_id, guest_id) = (host_endpoint.id(), guest_endpoint.id());
	host.share(host_endpoint, UserId(1)).expect("share");
	guest.connect(guest_endpoint, UserId(2)).expect("connect");
	network.connect(host_id);
	network.connect(guest_id);
	settle(&mut network, &mut [&mut host, &mut guest]);
	assert!(guest.is_synced());

	let id = ResourceId::from(5);
	guest.add_resource(id, b"guest bytes").expect("add");
	// The embedder answers for bytes the document does not serve itself.
	for _ in 0..16 {
		host.poll_peers();
		for event in guest.poll_peers() {
			if let Event::ResourceRequested { from, hash } = event {
				guest.send_resource(from, hash, b"guest bytes".to_vec()).expect("send");
			}
		}
		network.deliver_all();
	}
	assert!(host.session().registry().resources.contains_key(&id));
	assert!(
		futures::executor::block_on(host.has_resource(&ResourceHash::from(b"guest bytes".as_slice()))),
		"the bytes follow the entry"
	);
}

/// An interaction past a gap stays hot until the ops before it arrive, so it cannot go out ahead of them.
#[test]
fn an_interaction_past_a_gap_waits_to_retire() {
	let mut author = Session::with_peer(PeerId(2));
	for id in [1, 2] {
		author.stage_embedded_resource(ResourceId::from(id), ResourceHash::from(b"bytes".as_slice())).expect("stage");
		author.end_interaction().expect("close");
	}
	let ops = author.hot_log().to_vec();
	let first_end = ops.iter().position(HotOp::ends_interaction).expect("closed");
	let (earlier, later) = ops.split_at(first_end + 1);

	let mut host = document(1);
	assert!(host.apply_remote_hot_ops(later.to_vec()).expect("apply").is_empty());
	assert!(host.retire_pending_interaction().expect("retire").is_empty());
	assert_eq!(host.session().hot_log().len(), later.len());

	assert!(host.apply_remote_hot_ops(earlier.to_vec()).expect("apply").is_empty());
	assert!(!host.retire_pending_interaction().expect("retire").is_empty());
	assert!(host.session().hot_log().is_empty());
}

#[test]
fn the_user_cannot_change_while_connected() {
	let mut host = document(1);
	let mut network = MockNetwork::new(1);
	host.share(network.endpoint(), UserId(1)).expect("share");
	assert!(matches!(host.set_user(UserId(2), PeerId(2)), Err(document_format::Error::UserChangeWhileConnected)));
	assert_eq!(host.session().peer(), PeerId(1));
	assert_eq!(host.session().user(), UserId(1));
}

/// Edits to a large input reach a peer as deltas against what it already holds, and a peer joining later gets every blob with its sync.
#[test]
fn blobs_reach_peers_as_deltas_and_with_a_sync() {
	use document_container::AsyncContainer;
	use document_format::layout::Layout;
	use document_graph_storage::blob::BLOB_THRESHOLD;
	use document_graph_storage::{Implementation, Node, NodeId, NodeInput, ROOT_NETWORK, Value};

	let text = |length: usize| NodeInput::Value {
		value: Value::Str("a".repeat(length)),
		exposed: false,
	};
	let id = NodeId(1);

	let mut host = document(1);
	let mut guest = document(2);
	let mut network = MockNetwork::new(1);
	let (host_endpoint, guest_endpoint) = (network.endpoint(), network.endpoint());
	let (host_id, guest_id) = (host_endpoint.id(), guest_endpoint.id());
	host.share(host_endpoint, UserId(1)).expect("share");
	guest.connect(guest_endpoint, UserId(2)).expect("connect");
	network.connect(host_id);
	network.connect(guest_id);
	settle(&mut network, &mut [&mut host, &mut guest]);

	let mut node = Node::new(ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(1)), 1);
	node.inputs_mut()[0].input = text(BLOB_THRESHOLD);
	let root = RegistryDelta::AddNetwork {
		id: ROOT_NETWORK,
		network: Network::default(),
	};
	guest.stage_ops([root, RegistryDelta::AddNode { id, node }]).expect("add");
	for length in BLOB_THRESHOLD + 1..BLOB_THRESHOLD + 4 {
		guest
			.stage_ops([RegistryDelta::ChangeNodeInput {
				id,
				index: 0,
				new_input: text(length),
			}])
			.expect("edit");
	}
	settle(&mut network, &mut [&mut host, &mut guest]);

	let latest = guest.registry().node_instances[&id].inputs()[0].input.blob().expect("the input is a blob");
	assert_eq!(host.registry().node_instances[&id].inputs()[0].input.blob(), Some(latest));
	assert_eq!(host.blobs().len(), 4, "every version reached the host");
	assert!(host.session().all_blob_hashes().iter().all(|hash| host.blobs().contains_key(hash)));

	let late_endpoint = network.endpoint();
	let late_id = late_endpoint.id();
	let mut late = document(3);
	late.connect(late_endpoint, UserId(3)).expect("connect");
	network.connect(late_id);
	settle(&mut network, &mut [&mut host, &mut guest, &mut late]);
	assert!(late.is_synced());
	assert_eq!(late.blobs(), host.blobs(), "a sync carries every blob");

	let (working, layout) = host.into_storage();
	let record = futures::executor::block_on(working.read(&layout.blob_path(&latest))).expect("the host wrote the record");
	assert_eq!(record.as_slice()[0], 1, "an edit arrives as a delta");
	assert!(record.as_slice().len() < 100, "the delta is small, got {} bytes", record.as_slice().len());
}

/// A record whose base isn't held is dropped, and the blob is asked for in full like any missing resource.
#[test]
fn a_blob_without_its_base_is_asked_for_in_full() {
	use document_graph_storage::blob::BLOB_THRESHOLD;
	use document_graph_storage::{Implementation, Node, NodeId, NodeInput, ROOT_NETWORK, Value};

	let text = |length: usize| NodeInput::Value {
		value: Value::Str("a".repeat(length)),
		exposed: false,
	};
	let id = NodeId(1);

	let mut author = document(1);
	let mut node = Node::new(ROOT_NETWORK, Implementation::ProtoNode(ResourceId::from(1)), 1);
	node.inputs_mut()[0].input = text(BLOB_THRESHOLD);
	let root = RegistryDelta::AddNetwork {
		id: ROOT_NETWORK,
		network: Network::default(),
	};
	let mut hot_ops = author.stage_ops([root, RegistryDelta::AddNode { id, node }]).expect("add");
	hot_ops.extend(
		author
			.stage_ops([RegistryDelta::ChangeNodeInput {
				id,
				index: 0,
				new_input: text(BLOB_THRESHOLD + 1),
			}])
			.expect("edit"),
	);
	let latest = author.registry().node_instances[&id].inputs()[0].input.blob().expect("the input is a blob");
	// Blobs are fetched through the resource path, keyed by the same hash
	let resource = ResourceHash::from(latest);

	// Only the edit's delta record arrives, not the full record it is based on
	let mut reader = document(2);
	let records = author.blob_records().into_iter().filter(|record| record.hash == latest).collect();
	reader.apply_remote_hot_ops(hot_ops).expect("apply");
	reader.absorb_blob_records(records).expect("absorb");
	assert!(!reader.blobs().contains_key(&latest));
	assert!(reader.missing_resources().contains(&resource));

	let bytes = author.resource_bytes(resource).expect("the author serves the blob in full");
	reader.store_resource(resource, &bytes).expect("store");
	assert_eq!(reader.blobs().get(&latest), Some(&bytes));
	assert!(!reader.missing_resources().contains(&resource));
}
