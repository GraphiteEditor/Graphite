use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;

pub use graphene_core::uuid::*;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	pub dispatcher: Dispatcher,
}

impl Editor {
	/// Construct a new editor instance.
	/// Remember to provide a random seed with `editor::set_uuid_seed(seed)` before any editors can be used.
	pub fn new() -> Self {
		Self { dispatcher: Dispatcher::new() }
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message);

		std::mem::take(&mut self.dispatcher.responses)
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) {
		self.dispatcher.poll_node_graph_evaluation(responses);
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

pub fn release_series() -> String {
	format!("Release Series: {}", env!("GRAPHITE_RELEASE_SERIES"))
}

pub fn commit_info() -> String {
	format!("{}\n{}\n{}", commit_timestamp(), commit_hash(), commit_branch())
}

pub fn commit_info_localized(localized_commit_date: &str) -> String {
	format!("{}\n{}\n{}", commit_timestamp_localized(localized_commit_date), commit_hash(), commit_branch())
}

pub fn commit_timestamp() -> String {
	format!("Date: {}", env!("GRAPHITE_GIT_COMMIT_DATE"))
}

pub fn commit_timestamp_localized(localized_commit_date: &str) -> String {
	format!("Date: {}", localized_commit_date)
}

pub fn commit_hash() -> String {
	format!("Hash: {}", &env!("GRAPHITE_GIT_COMMIT_HASH")[..8])
}

pub fn commit_branch() -> String {
	format!("Branch: {}", env!("GRAPHITE_GIT_COMMIT_BRANCH"))
}

#[cfg(test)]
mod test {
	use crate::messages::{input_mapper::utility_types::input_mouse::ViewportBounds, prelude::*};

	#[test]
	fn debug_ub() {
		let mut editor = super::Editor::new();
		let mut responses = Vec::new();
		use super::Message::*;

		let messages: Vec<Message> = vec![
			Init,
			Preferences(
				PreferencesMessage::Load {
					preferences: r#"{"imaginate_server_hostname":"https://exchange-encoding-watched-insured.trycloudflare.com/","imaginate_refresh_frequency":1,"zoom_with_scroll":false}"#.to_string(),
				},
			),
			PortfolioMessage::OpenDocumentFileWithId {
				document_id: 0,
				document_name: "".into(),
				document_is_auto_saved: true,
				document_is_saved: true,
				document_serialized_content:
r#"
{"document_legacy":{"root":{"visible":true,"name":null,"data":{"Folder":{"next_assignment_id":12825788055422975214,"layer_ids":[12825788055422975213],"layers":[{"visible":true,"name":null,"data":{"Layer":{"network":{"inputs":[0],"outputs":[{"node_id":1,"node_output_index":0}],"nodes":{"0":{"name":"Input Frame","inputs":[{"Network":{"Concrete":{"name":"graphene_core::application_io::EditorApi<graphene_core::application_io::wasm_application_io::WasmApplicationIo>","size":80,"align":8}}}],"implementation":{"Network":{"inputs":[0],"outputs":[{"node_id":0,"node_output_index":0}],"nodes":{"0":{"name":"Input Frame_impl","inputs":[{"Network":{"Concrete":{"name":"graphene_core::application_io::EditorApi<graphene_core::application_io::wasm_application_io::WasmApplicationIo>","size":80,"align":8}}}],"implementation":{"Unresolved":{"name":"graphene_core::ExtractImageFrame"}},"metadata":{"position":[0,0]},"path":null}},"disabled":[],"previous_outputs":null}},"metadata":{"position":[8,4]},"path":null},"11577035356642256919":{"name":"Transform","inputs":[{"Node":{"node_id":0,"output_index":0,"lambda":false}},{"Value":{"tagged_value":{"DVec2":[703.2276466129997,473.0379249237632]},"exposed":false}},{"Value":{"tagged_value":{"F64":0.0},"exposed":false}},{"Value":{"tagged_value":{"DVec2":[345.616055733087,237.05356066324276]},"exposed":false}},{"Value":{"tagged_value":{"DVec2":[0.0,0.0]},"exposed":false}},{"Value":{"tagged_value":{"DVec2":[0.5,0.5]},"exposed":false}}],"implementation":{"Network":{"inputs":[0,0,0,0,0,0],"outputs":[{"node_id":0,"node_output_index":0}],"nodes":{"0":{"name":"Transform_impl","inputs":[{"Network":{"Concrete":{"name":"graphene_core::vector::vector_data::VectorData","size":248,"align":8}}},{"Network":{"Concrete":{"name":"glam::f64::dvec2::DVec2","size":16,"align":8}}},{"Network":{"Concrete":{"name":"f64","size":8,"align":8}}},{"Network":{"Concrete":{"name":"glam::f64::dvec2::DVec2","size":16,"align":8}}},{"Network":{"Concrete":{"name":"glam::f64::dvec2::DVec2","size":16,"align":8}}},{"Network":{"Concrete":{"name":"glam::f64::dvec2::DVec2","size":16,"align":8}}}],"implementation":{"Unresolved":{"name":"graphene_core::transform::TransformNode<_, _, _, _, _>"}},"metadata":{"position":[0,0]},"path":null}},"disabled":[],"previous_outputs":null}},"metadata":{"position":[16,4]},"path":null},"1":{"name":"Output","inputs":[{"Node":{"node_id":11577035356642256919,"output_index":0,"lambda":false}}],"implementation":{"Network":{"inputs":[0],"outputs":[{"node_id":0,"node_output_index":0}],"nodes":{"0":{"name":"Output_impl","inputs":[{"Network":{"Concrete":{"name":"graphene_core::raster::image::ImageFrame<graphene_core::raster::color::Color>","size":72,"align":8}}}],"implementation":{"Unresolved":{"name":"graphene_core::ops::IdNode"}},"metadata":{"position":[0,0]},"path":null}},"disabled":[],"previous_outputs":null}},"metadata":{"position":[24,4]},"path":null}},"disabled":[],"previous_outputs":null}}},"transform":{"matrix2":[345.616055733087,0.0,-0.0,237.05356066324276],"translation":[530.919618746456,355.01114459214176]},"preserve_aspect":true,"pivot":[0.5,0.5],"blend_mode":"Normal","opacity":1.0}]}},"transform":{"matrix2":[0.5833333598242877,0.0,0.0,0.5833333598242877],"translation":[11.0,214.99999999999994]},"preserve_aspect":true,"pivot":[0.5,0.5],"blend_mode":"Normal","opacity":1.0},"document_network":{"inputs":[],"outputs":[{"node_id":0,"node_output_index":0}],"nodes":{"0":{"name":"Output","inputs":[{"Value":{"tagged_value":{"GraphicGroup":[]},"exposed":true}}],"implementation":{"Unresolved":{"name":"graphene_core::ops::IdNode"}},"metadata":{"position":[8,4]},"path":null}},"disabled":[],"previous_outputs":null}},"saved_document_identifier":0,"auto_saved_document_identifier":0,"name":"Untitled Document","version":"0.0.16","document_mode":"DesignMode","view_mode":"Normal","snapping_enabled":true,"overlays_visible":true,"layer_metadata":[[[],{"selected":false,"expanded":true}],[[12825788055422975213],{"selected":false,"expanded":false}]],"layer_range_selection_reference":[],"navigation_handler":{"pan":[-960.0,-540.5],"panning":false,"snap_tilt":false,"snap_tilt_released":false,"tilt":0.0,"tilting":false,"zoom":0.5833333598242877,"zooming":false,"snap_zoom":false,"mouse_position":[0.0,0.0]},"artboard_message_handler":{"artboards_document":{"root":{"visible":true,"name":null,"data":{"Folder":{"next_assignment_id":17677129199720758749,"layer_ids":[17677129199720758748],"layers":[{"visible":true,"name":null,"data":{"Shape":{"shape":{"elements":[{"points":[{"position":[0.0,0.0],"manipulator_type":"Anchor"},null,null]},{"points":[{"position":[0.0,1.0],"manipulator_type":"Anchor"},null,null]},{"points":[{"position":[1.0,1.0],"manipulator_type":"Anchor"},null,null]},{"points":[{"position":[1.0,0.0],"manipulator_type":"Anchor"},null,null]},{"points":[null,null,null]}],"element_ids":[1,2,3,4,5],"next_id":5},"style":{"stroke":null,"fill":{"Solid":{"red":1.0,"green":1.0,"blue":1.0,"alpha":1.0}}},"render_index":1}},"transform":{"matrix2":[1920.0,0.0,-0.0,1080.0],"translation":[0.0,0.0]},"preserve_aspect":true,"pivot":[0.5,0.5],"blend_mode":"Normal","opacity":1.0}]}},"transform":{"matrix2":[0.5833333598242877,0.0,0.0,0.5833333598242877],"translation":[11.0,214.99999999999994]},"preserve_aspect":true,"pivot":[0.5,0.5],"blend_mode":"Normal","opacity":1.0},"document_network":{"inputs":[],"outputs":[{"node_id":0,"node_output_index":0}],"nodes":{"0":{"name":"Output","inputs":[{"Value":{"tagged_value":{"GraphicGroup":[]},"exposed":true}}],"implementation":{"Unresolved":{"name":"graphene_core::ops::IdNode"}},"metadata":{"position":[8,4]},"path":null}},"disabled":[],"previous_outputs":null}},"artboard_ids":[17677129199720758748]},"properties_panel_message_handler":{"active_selection":null}}
"#.into(),
			}.into(),
			InputPreprocessorMessage::BoundsOfViewports{bounds_of_viewports: vec![ViewportBounds::from_slice(&[0., 0., 1920., 1080.])]}.into(),
		];

		use futures::executor::block_on;
		for message in messages {
			block_on(crate::node_graph_executor::run_node_graph());
			let mut res = VecDeque::new();
			editor.poll_node_graph_evaluation(&mut res);
			//println!("node_graph_poll: {:#?}", res);

			//println!("in: {:#?}", message);
			let res = editor.handle_message(message);
			//println!("out: {:#?}", res);
			responses.push(res);
		}
		let responses = responses.pop().unwrap();
		let trigger_message = responses[responses.len() - 2].clone();
		if let FrontendMessage::TriggerRasterizeRegionBelowLayer { size, .. } = trigger_message {
			assert!(size.x > 0. && size.y > 0.);
		} else {
			panic!();
		}
		println!("responses: {:#?}", responses);
	}
}
