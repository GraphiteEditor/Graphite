// History metadata: persisted with the working copy and merged with every copy met.
// Needs the network feature for a session and the runtime conversion for staging ops.
#![cfg(all(feature = "conversion", feature = "network"))]

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use document_graph_storage::from_runtime::DeclarationBytes;
use document_graph_storage::{Network, NetworkId, PeerId, RegistryDelta, Rev, TimeStamp, UserId, rev_attr, user_attr};
use peer_transport::mock::MockNetwork;

const NOW_MS: f64 = 1_700_000_000_000.;

async fn document(peer: u64, user: u64) -> GddV1 {
	GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(peer), UserId(user), 1, "editor".into(), "stdlib".into()).expect("create")
}

/// Let the room settle: greetings, the sync handshake and whatever was broadcast.
fn settle(network: &mut MockNetwork, peers: &mut [&mut GddV1]) {
	for _ in 0..16 {
		for peer in peers.iter_mut() {
			peer.poll_peers();
		}
		network.deliver_all();
	}
	for peer in peers.iter_mut() {
		peer.poll_peers();
	}
}

#[test]
fn a_recorded_name_survives_a_reopen() {
	futures::executor::block_on(async {
		let mut gdd = document(1, 10).await;
		assert!(gdd.record_user_attribute(UserId(10), user_attr::NAME, "Ada".into(), NOW_MS).expect("record"));
		assert!(
			!gdd.record_user_attribute(UserId(10), user_attr::NAME, "Ada".into(), NOW_MS + 1.).expect("record"),
			"the same name is no change"
		);

		let step = Rev::new(7).expect("non-zero");
		assert!(gdd.record_rev_attribute(step, rev_attr::LABEL, "Fixed the eye".into(), NOW_MS).expect("record"));
		assert!(gdd.record_rev_attribute(step, "tag:v1", true.into(), NOW_MS).expect("record"));
		let (working, layout) = gdd.into_storage();
		let reopened = GddV1::open_in(working, layout).await.expect("open");
		assert_eq!(reopened.metadata().user_name(UserId(10)), Some("Ada"));
		assert_eq!(reopened.metadata().rev_label(step), Some("Fixed the eye"), "a label appended after the name is folded back in");
		assert_eq!(reopened.metadata().rev_tags(step), vec!["v1"]);
	});
}

#[test]
fn names_reach_the_room_with_the_sync_and_as_they_change() {
	futures::executor::block_on(async {
		let mut host = document(1, 10).await;
		let mut guest = document(2, 20).await;
		host.record_user_attribute(UserId(10), user_attr::NAME, "Ada".into(), NOW_MS).expect("record");
		// A name given before joining travels with the joiner and reaches the host.
		guest.record_user_attribute(UserId(20), user_attr::NAME, "Bea".into(), NOW_MS).expect("record");

		let mut network = MockNetwork::new(1);
		let host_endpoint = network.endpoint();
		let guest_endpoint = network.endpoint();
		let (host_id, guest_id) = (host_endpoint.id(), guest_endpoint.id());
		host.share(host_endpoint, UserId(10));
		guest.join(guest_endpoint, UserId(20));
		network.connect(host_id);
		network.connect(guest_id);
		settle(&mut network, &mut [&mut host, &mut guest]);

		assert!(guest.is_synced());
		assert_eq!(guest.metadata().user_name(UserId(10)), Some("Ada"), "the host's record came with the sync");
		assert_eq!(host.metadata().user_name(UserId(20)), Some("Bea"), "the guest's record came back to the host");

		// A rename while in the room goes round on its own.
		assert!(guest.record_user_attribute(UserId(20), user_attr::NAME, "Beatrix".into(), NOW_MS + 5_000.).expect("record"));
		settle(&mut network, &mut [&mut host, &mut guest]);
		assert_eq!(host.metadata().user_name(UserId(20)), Some("Beatrix"));
		assert_eq!(host.metadata(), guest.metadata(), "both copies hold the same record");
	});
}

#[test]
fn a_retirement_records_when_it_happened() {
	futures::executor::block_on(async {
		let mut gdd = document(1, 10).await;
		// The editor's per-frame tick is where the wall clock comes from.
		gdd.retire_due(NOW_MS, true).expect("tick");

		let byte_store = graph_craft::application_io::resource::HashMapResourceStorage::new();
		gdd.stage_constructed_ops(
			vec![RegistryDelta::AddNetwork {
				id: NetworkId(9),
				network: Network::default(),
			}],
			&DeclarationBytes::default(),
			&byte_store,
		)
		.expect("stage");
		let everything = TimeStamp {
			counter: u64::MAX,
			peer: PeerId(u64::MAX),
		};
		let revs = gdd.retire(everything).expect("retire");
		assert!(!revs.is_empty());
		for rev in revs {
			let delta = gdd.session().delta(rev).expect("retired delta");
			assert_eq!(delta.retired_at(), Some(NOW_MS as u64));
		}
	});
}
