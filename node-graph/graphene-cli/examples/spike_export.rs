//! Exports the compiler spike's workloads from documents opened the way the editor opens them.
//!
//! Usage: `cargo run -p graphene-cli --example spike_export -- <output directory> <documents or directories>...`

use graph_craft::application_io::resource::ResourceId;
use graphene_cli::open_document;
use preprocessor::spike_export::{WorkloadDocument, export_compiler_spike_workloads};
use std::path::PathBuf;

fn documents_in(paths: &[PathBuf]) -> Vec<PathBuf> {
	let mut documents = Vec::new();
	for path in paths {
		if !path.is_dir() {
			documents.push(path.clone());
			continue;
		}

		let mut found: Vec<PathBuf> = std::fs::read_dir(path)
			.into_iter()
			.flatten()
			.filter_map(Result::ok)
			.map(|entry| entry.path())
			.filter(|path| path.extension().is_some_and(|extension| extension == "gdd" || extension == "graphite"))
			.collect();
		found.sort();
		documents.extend(found);
	}
	documents
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut arguments = std::env::args_os().skip(1).map(PathBuf::from);
	let output_directory = arguments.next().ok_or("expected an output directory")?;
	let inputs: Vec<PathBuf> = arguments.collect();

	let mut opened = Vec::new();
	for path in documents_in(&inputs) {
		let name = path.file_stem().and_then(|stem| stem.to_str()).ok_or("document names must be UTF-8")?.to_string();
		let mut document = open_document(&path, false).await.map_err(|error| format!("{}: {error}", path.display()))?;

		// The editor lifts nested networks into the definitions table on load, which a legacy document still nests inline
		document.network.lift_nested_networks();
		opened.push((name, document));
	}

	let documents = opened.iter().map(|(name, document)| WorkloadDocument {
		name: name.clone(),
		network: document.network.clone(),
		resolve_resource: Box::new(move |resource_id: ResourceId| document.gdd.as_ref()?.registry().resources.get(&resource_id).and_then(|resource| resource.hash)),
	});
	export_compiler_spike_workloads(&output_directory, documents)?;

	println!("Exported {} documents to {}", opened.len(), output_directory.display());
	Ok(())
}
