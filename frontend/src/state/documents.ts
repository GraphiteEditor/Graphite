/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import {
	DisplayConfirmationToCloseAllDocuments,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	FrontendDocumentDetails,
	OpenDocumentBrowse,
	SaveDocument,
	SetActiveDocument,
	UpdateOpenDocumentsList,
} from "@/dispatcher/js-messages";
import { DialogState } from "@/state/dialog";
import { EditorState } from "@/state/wasm-loader";
import { download, upload } from "@/utilities/files";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentsState(editor: EditorState, dialogState: DialogState) {
	const state = reactive({
		unsaved: false,
		documents: [] as FrontendDocumentDetails[],
		activeDocumentIndex: 0,
	});

	const closeDocumentWithConfirmation = async (documentId: BigInt): Promise<void> => {
		// Assume we receive a correct document_id
		const targetDocument = state.documents.find((doc) => doc.id === documentId) as FrontendDocumentDetails;
		const tabLabel = targetDocument.displayName;

		// Show the close confirmation prompt
		dialogState.createDialog("File", "Save changes before closing?", tabLabel, [
			{
				kind: "TextButton",
				callback: async (): Promise<void> => {
					editor.instance.save_document();
					dialogState.dismissDialog();
				},
				props: { label: "Save", emphasized: true, minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: async (): Promise<void> => {
					editor.instance.close_document(targetDocument.id);
					dialogState.dismissDialog();
				},
				props: { label: "Discard", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: async (): Promise<void> => {
					dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	};

	const closeAllDocumentsWithConfirmation = (): void => {
		dialogState.createDialog("Copy", "Close all documents?", "Unsaved work will be lost!", [
			{
				kind: "TextButton",
				callback: (): void => {
					editor.instance.close_all_documents();
					dialogState.dismissDialog();
				},
				props: { label: "Discard All", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: (): void => {
					dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	};

	// Set up message subscriptions on creation
	editor.dispatcher.subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
		state.documents = updateOpenDocumentList.open_documents;
	});

	editor.dispatcher.subscribeJsMessage(SetActiveDocument, (setActiveDocument) => {
		// Assume we receive a correct document id
		const activeId = state.documents.findIndex((doc) => doc.id === setActiveDocument.document_id);
		state.activeDocumentIndex = activeId;
	});

	editor.dispatcher.subscribeJsMessage(DisplayConfirmationToCloseDocument, (displayConfirmationToCloseDocument) => {
		closeDocumentWithConfirmation(displayConfirmationToCloseDocument.document_id);
	});

	editor.dispatcher.subscribeJsMessage(DisplayConfirmationToCloseAllDocuments, () => {
		closeAllDocumentsWithConfirmation();
	});

	editor.dispatcher.subscribeJsMessage(OpenDocumentBrowse, async () => {
		const extension = editor.rawWasm.file_save_suffix();
		const data = await upload(extension);
		editor.instance.open_document_file(data.filename, data.content);
	});

	editor.dispatcher.subscribeJsMessage(ExportDocument, (exportDocument) => {
		download(exportDocument.name, exportDocument.document);
	});

	editor.dispatcher.subscribeJsMessage(SaveDocument, (saveDocument) => {
		download(saveDocument.name, saveDocument.document);
	});

	// Get the initial documents
	editor.instance.get_open_documents_list();

	return {
		state: readonly(state),
		closeAllDocumentsWithConfirmation,
	};
}
export type DocumentsState = ReturnType<typeof createDocumentsState>;
