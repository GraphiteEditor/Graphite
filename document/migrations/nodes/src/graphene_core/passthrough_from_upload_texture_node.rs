use migration_core::{Inputs, migration};

/// 'Upload Texture' is gone along with the GPU adjustment nodes it fed, so it passes its content on and drops the executor input.
#[migration("graphene_std::platform_application_io::UploadTextureNode"@1 => "graphene_core::ops::PassthroughNode"@2)]
pub fn passthrough_from_upload_texture_node(inputs: &mut Inputs) {
	inputs.truncate(1);
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::{decl, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(2, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&PassthroughFromUploadTextureNode, &contract());
		check_undo(&PassthroughFromUploadTextureNode, &contract());
	}

	#[test]
	fn keeps_the_content_and_drops_the_executor() {
		let (content, executor) = (literal(1.), literal(2.));

		let mut fixture = Fixture::new(&PassthroughFromUploadTextureNode);
		let upload_texture = fixture.add(decl!("graphene_std::platform_application_io::UploadTextureNode"@1), vec![content.clone(), executor]);
		let migrated = fixture.migrate(&PassthroughFromUploadTextureNode).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, upload_texture), Some(decl!("graphene_core::ops::PassthroughNode"@2)));
		assert_eq!(fixture.inputs(&migrated, upload_texture), [content]);
	}
}
