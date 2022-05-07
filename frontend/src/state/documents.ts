/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { TriggerFileDownload, FrontendDocumentDetails, TriggerFileUpload, UpdateActiveDocument, UpdateOpenDocumentsList } from "@/dispatcher/js-messages";
import { EditorState } from "@/state/wasm-loader";
import { download, upload } from "@/utilities/files";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentsState(editor: EditorState) {
	const state = reactive({
		unsaved: false,
		documents: [] as FrontendDocumentDetails[],
		activeDocumentIndex: 0,
	});

	// Set up message subscriptions on creation
	editor.dispatcher.subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
		state.documents = updateOpenDocumentList.open_documents;
	});

	editor.dispatcher.subscribeJsMessage(UpdateActiveDocument, (updateActiveDocument) => {
		// Assume we receive a correct document id
		const activeId = state.documents.findIndex((doc) => doc.id === updateActiveDocument.document_id);
		state.activeDocumentIndex = activeId;
	});

	editor.dispatcher.subscribeJsMessage(TriggerFileUpload, async () => {
		const extension = editor.rawWasm.file_save_suffix();
		const data = await upload(extension);
		editor.instance.open_document_file(data.filename, data.content);
	});

	editor.dispatcher.subscribeJsMessage(TriggerFileDownload, (triggerFileDownload) => {
		download(triggerFileDownload.name, triggerFileDownload.document);
	});

	// TODO(mfish33): Replace with initialization system Issue:#524
	// Get the initial documents
	editor.instance.get_open_documents_list();

	return {
		state: readonly(state),
	};
}
export type DocumentsState = ReturnType<typeof createDocumentsState>;
