//! Opens a document through the real load path, which migrates its large node input values into blobs, and reports the stored sizes.

use std::path::Path;

use document_container::{AnyContainer, AsyncContainer, backends::memory::MemoryBackend};
use document_format::export::{ExportFormat, ExportOptions};
use document_format::layout::Layout;
use document_format::{Gdd, GddV1Layout};

// Format steps register when linked
use migration_formats as _;

fn main() {
	let document_path = std::env::args().nth(1).unwrap_or_else(|| "Brush Big.gdd".to_string());
	let archive = std::fs::read(Path::new(&document_path)).unwrap();

	futures::executor::block_on(async {
		let gdd = Gdd::open_from_archive(archive.as_ref(), AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.unwrap();
		let (working, layout) = gdd.into_storage();

		let blob_paths = working.list(layout.blobs_dir()).await.unwrap_or_default();
		let mut blob_total = 0;
		let mut delta_count = 0;
		let mut full_total = 0;
		for path in &blob_paths {
			let record = working.read(path).await.unwrap();
			blob_total += record.as_slice().len();
			delta_count += usize::from(record.as_slice().first() == Some(&1));
			if record.as_slice().first() == Some(&0) {
				full_total += record.as_slice().len();
			}
		}

		println!("source archive (.gdd):         {:>9} bytes", archive.len());
		for path in working.list("").await.unwrap_or_default().iter().filter(|path| !path.starts_with(layout.blobs_dir())) {
			let bytes = working.read(path).await.map(|bytes| bytes.as_slice().len()).unwrap_or_default();
			println!("{path:<31}{bytes:>9} bytes");
		}
		println!("blobs/: {:>5} records ({delta_count} deltas) {:>9} bytes", blob_paths.len(), blob_total);
		println!("  of which full records:        {full_total:>9} bytes");

		let gdd = Gdd::open_in(working, layout).await.unwrap();
		for (label, format) in [("zip", ExportFormat::Zip), ("xz", ExportFormat::Xz)] {
			let exported = gdd.export_to_bytes(format, ExportOptions::default(), &gdd, None).await.unwrap();
			println!("export ({label}):                  {:>9} bytes", exported.len());
		}
	});
}
