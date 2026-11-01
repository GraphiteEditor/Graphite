// History metadata is appended to its own file with the working copy, carried by an export and merged with every copy met.

use document_container::backends::memory::MemoryBackend;
use document_container::{AnyContainer, AsyncContainer};
use document_format::{GddV1, GddV1Layout, Layout};
use document_graph_storage::{Network, NetworkId, PeerId, RegistryDelta, Rev, Subject, TimeStamp, UserId, rev_attr, user_attr};

const NOW_MS: f64 = 1_700_000_000_000.;

fn document(peer: u64, user: u64) -> GddV1 {
	GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(peer), UserId(user), 1, "editor".into(), "stdlib".into()).expect("create")
}

/// Let the room settle through greetings, the sync handshake and whatever was broadcast.
#[cfg(feature = "network")]
fn settle(network: &mut peer_transport::mock::MockNetwork, peers: &mut [&mut GddV1]) {
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
		let mut gdd = document(1, 10);
		assert!(gdd.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS).expect("record"));
		assert!(
			!gdd.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS + 1.).expect("record"),
			"the same name is no change"
		);

		let step = Rev::new(7).expect("non-zero");
		assert!(gdd.record(Subject::Rev(step), rev_attr::LABEL, "Fixed the eye".into(), NOW_MS).expect("record"));
		assert!(gdd.record(Subject::Rev(step), "tag:v1", true.into(), NOW_MS).expect("record"));
		let (working, layout) = gdd.into_storage();
		let mut reopened = GddV1::open_in(working, layout).await.expect("open");
		assert_eq!(reopened.metadata().user_name(UserId(10)), Some("Ada"));

		// A fact recorded after a reopen joins the ones already there.
		reopened.record(Subject::User(UserId(11)), user_attr::NAME, "Bea".into(), NOW_MS).expect("record");
		let (working, layout) = reopened.into_storage();
		let reopened = GddV1::open_in(working, layout).await.expect("open");
		assert_eq!(reopened.metadata().user_name(UserId(11)), Some("Bea"));
		assert_eq!(reopened.metadata().rev_label(step), Some("Fixed the eye"), "a label appended after the name is folded back in");
		assert_eq!(reopened.metadata().rev_tags(step), vec!["v1"]);
	});
}

#[test]
fn recording_an_unchanged_value_writes_nothing() {
	futures::executor::block_on(async {
		let mut gdd = document(1, 10);
		gdd.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS).expect("record");
		let metadata = gdd.metadata().clone();
		let (working, layout) = gdd.into_storage();
		let path = document_format::io::path_for(layout.metadata_basename(), document_format::METADATA_CODEC);
		let before = working.read(&path).await.expect("read").as_slice().to_vec();

		let mut reopened = GddV1::open_in(working, layout).await.expect("open");
		assert!(!reopened.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS + 1.).expect("record"));
		assert_eq!(reopened.metadata(), &metadata);
		let (working, _) = reopened.into_storage();
		assert_eq!(working.read(&path).await.expect("read").as_slice().trim_ascii(), before.trim_ascii());
	});
}

/// A line that doesn't decode, such as one torn by a crash or a kind of fact from a newer editor, is skipped rather
/// than keeping the document from opening.
#[test]
fn an_unreadable_metadata_line_is_skipped() {
	futures::executor::block_on(async {
		let mut gdd = document(1, 10);
		gdd.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS).expect("record");
		let (working, layout) = gdd.into_storage();
		let path = document_format::io::path_for(layout.metadata_basename(), document_format::METADATA_CODEC);
		working
			.append_non_blocking(
				&path,
				b"{\"subject\":{\"Branch\":3},\"key\":\"name\",\"value\":1,\"stamp\":{\"ms\":1,\"peer\":1}}\n{\"subject\":{\"User\":",
			)
			.expect("append");

		let reopened = GddV1::open_in(working, layout).await.expect("open");
		assert_eq!(reopened.metadata().user_name(UserId(10)), Some("Ada"));
	});
}

#[test]
fn a_torn_tail_is_skipped_and_the_next_fact_is_not_joined_to_it() {
	futures::executor::block_on(async {
		let mut gdd = document(1, 10);
		gdd.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS).expect("record");
		let (working, layout) = gdd.into_storage();
		let path = document_format::io::path_for(layout.metadata_basename(), document_format::METADATA_CODEC);
		working.append_non_blocking(&path, b"{\"subject\":{\"User\":").expect("append");

		let mut reopened = GddV1::open_in(working, layout).await.expect("open");
		reopened.record(Subject::User(UserId(11)), user_attr::NAME, "Bea".into(), NOW_MS).expect("record");
		let (working, layout) = reopened.into_storage();
		let reopened = GddV1::open_in(working, layout).await.expect("open");
		assert_eq!(reopened.metadata().user_name(UserId(10)), Some("Ada"));
		assert_eq!(reopened.metadata().user_name(UserId(11)), Some("Bea"));
	});
}

#[cfg(feature = "network")]
#[test]
fn names_reach_the_room_with_the_sync_and_as_they_change() {
	futures::executor::block_on(async {
		let mut host = document(1, 10);
		let mut guest = document(2, 20);
		host.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS).expect("record");
		// A name given before joining travels with the joiner and reaches the host.
		guest.record(Subject::User(UserId(20)), user_attr::NAME, "Bea".into(), NOW_MS).expect("record");

		let mut network = peer_transport::mock::MockNetwork::new(1);
		let host_endpoint = network.endpoint();
		let guest_endpoint = network.endpoint();
		let (host_id, guest_id) = (host_endpoint.id(), guest_endpoint.id());
		host.share(host_endpoint, UserId(10)).expect("share");
		guest.connect(guest_endpoint, UserId(20)).expect("connect");
		network.connect(host_id);
		network.connect(guest_id);
		settle(&mut network, &mut [&mut host, &mut guest]);

		assert!(guest.is_synced());
		assert_eq!(guest.metadata().user_name(UserId(10)), Some("Ada"), "the host's record came with the sync");
		assert_eq!(host.metadata().user_name(UserId(20)), Some("Bea"), "the guest's record came back to the host");

		// A rename while in the room goes round on its own.
		assert!(guest.record(Subject::User(UserId(20)), user_attr::NAME, "Beatrix".into(), NOW_MS + 5_000.).expect("record"));
		settle(&mut network, &mut [&mut host, &mut guest]);
		assert_eq!(host.metadata().user_name(UserId(20)), Some("Beatrix"));
		assert_eq!(host.metadata(), guest.metadata(), "both copies hold the same record");
	});
}

#[test]
fn a_retirement_records_when_it_happened() {
	futures::executor::block_on(async {
		let mut gdd = document(1, 10);
		gdd.retire_due(NOW_MS, true).expect("tick");

		gdd.stage_ops([RegistryDelta::AddNetwork {
			id: NetworkId(9),
			network: Network::default(),
		}])
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

/// An export with history carries the facts about it. One without history carries none, since they'd name revs it lacks.
#[cfg(all(feature = "conversion", feature = "zip"))]
#[test]
fn an_export_carries_the_metadata_only_with_the_history() {
	futures::executor::block_on(async {
		let mut gdd = document(1, 10);
		gdd.record(Subject::User(UserId(10)), user_attr::NAME, "Ada".into(), NOW_MS).expect("record");

		let byte_store = graph_craft::application_io::resource::HashMapResourceStorage::new();
		for include_history in [true, false] {
			let options = document_format::ExportOptions {
				include_history,
				..Default::default()
			};
			let archive = gdd.export_to_bytes(document_format::ExportFormat::Zip, options, &byte_store, None).await.expect("export");
			let reopened = GddV1::open_from_archive(&archive, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.expect("open");
			let expected = include_history.then_some("Ada");
			assert_eq!(reopened.metadata().user_name(UserId(10)), expected, "include_history: {include_history}");
		}
	});
}
