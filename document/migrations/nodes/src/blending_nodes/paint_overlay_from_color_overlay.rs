use migration_core::attr::node::ui::DISPLAY_NAME;
use migration_core::{Attributes, Node, migration};

/// 'Color Overlay' became the 'Paint Overlay' effect, which takes its color as a solid paint. A name it stored as its default would misname it.
#[migration("raster_nodes::blending_nodes::ColorOverlayNode"@1 => "blending_nodes::effects::PaintOverlayNode"@2)]
pub fn paint_overlay_from_color_overlay(node: &Node, attributes: &mut Attributes) {
	if node.attributes().get_typed::<String>(DISPLAY_NAME).as_deref() == Some("Color Overlay") {
		attributes.remove(node.id(), DISPLAY_NAME);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use document_graph_storage::TimeStamp;
	use migration_core::{decl, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	#[test]
	fn rewrites_history() {
		let contract = SourceContract {
			display_names: &["Color Overlay"],
			..SourceContract::new(4, |random, _| literal(random.below(10) as f64))
		};
		check_commutation(&PaintOverlayFromColorOverlay, &contract);
		check_undo(&PaintOverlayFromColorOverlay, &contract);
	}

	#[test]
	fn only_the_old_default_name_is_cleared() {
		for (name, kept) in [("Color Overlay", None), ("Tint", Some("Tint"))] {
			let mut fixture = Fixture::new(&PaintOverlayFromColorOverlay);
			let overlay = fixture.add(decl!("raster_nodes::blending_nodes::ColorOverlayNode"@1), Vec::new());
			let attributes = fixture.registry.node_instances.get_mut(&overlay).unwrap().attributes_mut();
			attributes.set_serialized(DISPLAY_NAME, &name, TimeStamp::ORIGIN).unwrap();

			let migrated = fixture.migrate(&PaintOverlayFromColorOverlay).unwrap();

			assert_eq!(fixture.declaration_of(&migrated, overlay), Some(decl!("blending_nodes::effects::PaintOverlayNode"@2)));
			assert_eq!(migrated.node_instances[&overlay].attributes().get_typed::<String>(DISPLAY_NAME).as_deref(), kept);
		}
	}
}
