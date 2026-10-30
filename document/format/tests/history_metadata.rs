use document_container::backends::memory::MemoryBackend;
use document_container::{AnyContainer, AsyncContainer};
use document_format::{GddV1, GddV1Layout, Layout};
use document_graph_storage::{PeerId, Rev, Subject, UserId, rev_attr, user_attr};

const NOW_MS: f64 = 1_700_000_000_000.;
const ADA: Subject = Subject::User(UserId(10));

fn document_naming_ada() -> GddV1 {
	let mut gdd = GddV1::create_in(AnyContainer::Memory(MemoryBackend::new()), GddV1Layout, PeerId(1), 1, "editor".into(), "stdlib".into()).expect("create");
	assert!(gdd.record(ADA, user_attr::NAME, "Ada".into(), NOW_MS).expect("record"));
	gdd
}

#[test]
fn recorded_facts_survive_a_reopen() {
	futures::executor::block_on(async {
		let mut gdd = document_naming_ada();
		assert!(!gdd.record(ADA, user_attr::NAME, "Ada".into(), NOW_MS + 1.).expect("record"), "the same name is no change");
		let step = Rev::new(7).expect("non-zero");
		gdd.record(Subject::Rev(step), rev_attr::LABEL, "Fixed the eye".into(), NOW_MS).expect("record");

		let (working, layout) = gdd.into_storage();
		let reopened = GddV1::open_in(working, layout).await.expect("open");
		assert_eq!(reopened.metadata().user_name(UserId(10)), Some("Ada"));
		assert_eq!(reopened.metadata().rev_label(step), Some("Fixed the eye"));
	});
}

#[cfg(all(feature = "conversion", feature = "zip"))]
#[test]
fn an_export_carries_the_metadata_only_with_the_history() {
	futures::executor::block_on(async {
		let gdd = document_naming_ada();
		let byte_store = graph_craft::application_io::resource::HashMapResourceStorage::new();
		for include_history in [true, false] {
			let options = document_format::ExportOptions {
				include_history,
				..Default::default()
			};
			let archive = gdd.export_to_bytes(document_format::ExportFormat::Zip, options, &byte_store, None).await.expect("export");
			let reopened = GddV1::open_from_archive(&archive, AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.expect("open");
			assert_eq!(reopened.metadata().user_name(UserId(10)), include_history.then_some("Ada"));
		}
	});
}

#[test]
fn a_torn_tail_is_skipped_and_the_next_fact_is_not_joined_to_it() {
	futures::executor::block_on(async {
		let (working, layout) = document_naming_ada().into_storage();
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
