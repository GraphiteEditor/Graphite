#![cfg(feature = "network")]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::{Network, NetworkId, PeerId, RegistryDelta, ResourceId, UserId};
use peer_transport::mock::MockNetwork;

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
	host.share(host_endpoint, UserId(1));
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
	host.share(host_endpoint, UserId(1));
	guest.join(guest_endpoint, UserId(2));
	network.connect(host_id);
	network.connect(guest_id);
	settle(&mut network, &mut [&mut host, &mut guest]);
	assert!(guest.is_synced());

	let id = ResourceId::from(5);
	guest.add_resource(id, b"guest bytes").expect("add");
	settle(&mut network, &mut [&mut host, &mut guest]);
	assert!(host.session().registry().resources.contains_key(&id));
}
