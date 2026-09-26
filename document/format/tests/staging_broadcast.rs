// Needs the runtime conversion for `stage_constructed_ops` and the network feature for a session.
#![cfg(all(feature = "conversion", feature = "network"))]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::from_runtime::DeclarationBytes;
use document_graph_storage::{Network, NetworkId, PeerId, RegistryDelta, UserId};
use peer_transport::mock::MockNetwork;

/// Every staging path has to reach the room. The recorded-batch path once appended its hot frames and
/// stopped there, so an edit made through the interface was persisted and never sent.
#[test]
fn staging_a_constructed_batch_reaches_the_room() {
	futures::executor::block_on(async {
		let mut host = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), UserId(1), 1, "editor".into(), "stdlib".into())
			.await
			.expect("create");

		let mut network = MockNetwork::new(1);
		let host_endpoint = network.endpoint();
		let guest_endpoint = network.endpoint();
		let (host_id, guest_id) = (host_endpoint.id(), guest_endpoint.id());
		host.share(host_endpoint, UserId(1));
		network.connect(host_id);
		network.connect(guest_id);

		// Greeting the guest is what lets the host send to it; the hello itself is then out of the way.
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
				&DeclarationBytes::default(),
				&byte_store,
			)
			.expect("stage");

		assert!(!staged.is_empty(), "the staged hot ops come back for the caller");
		assert!(network.pending() > 0, "a staged batch must be broadcast to the room");
	});
}
