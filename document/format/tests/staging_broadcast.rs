#![cfg(feature = "network")]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{HotOp, Network, NetworkId, PeerId, RegistryDelta, ResourceId, Session, UserId};
use graphene_resource::ResourceHash;
use peer_transport::mock::MockNetwork;
use peer_transport::{Event, SyncTarget};

fn document(peer: u64) -> GddV1 {
	GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(peer), 1, "editor".into(), "stdlib".into()).expect("create")
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
