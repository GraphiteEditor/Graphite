import { reactive, readonly } from "vue";

import { createDialog, dismissDialog } from "@/utilities/dialog";
import { ResponseType, registerResponseHandler, Response, SetActiveDocument, UpdateOpenDocumentsList, DisplayConfirmationToCloseDocument } from "@/utilities/response-handler";

const wasm = import("@/../wasm/pkg");

const state = reactive({
	title: "",
	unsaved: false,
	documents: [] as Array<string>,
	activeDocumentIndex: 0,
});

export async function selectDocument(tabIndex: number) {
	(await wasm).select_document(tabIndex);
}

export async function closeDocumentWithConfirmation(tabIndex: number) {
	selectDocument(tabIndex);

	const tabLabel = state.documents[tabIndex];

	// TODO: Rename to "Save changes before closing?" when we can actually save documents somewhere, not just export SVGs
	createDialog("File", "Close without exporting SVG?", tabLabel, [
		{
			kind: "TextButton",
			callback: async () => {
				(await wasm).export_document();
				dismissDialog();
			},
			props: { label: "Export", emphasized: true, minWidth: 96 },
		},
		{
			kind: "TextButton",
			callback: async () => {
				(await wasm).close_document(tabIndex);
				dismissDialog();
			},
			props: { label: "Discard", minWidth: 96 },
		},
		{
			kind: "TextButton",
			callback: async () => {
				dismissDialog();
			},
			props: { label: "Cancel", minWidth: 96 },
		},
	]);
}

export async function closeAllDocumentsWithConfirmation() {
	createDialog("Copy", "Close all documents?", "Unsaved work will be lost!", [
		{
			kind: "TextButton",
			callback: async () => {
				(await wasm).close_all_documents();
				dismissDialog();
			},
			props: { label: "Discard All", minWidth: 96 },
		},
		{
			kind: "TextButton",
			callback: async () => {
				dismissDialog();
			},
			props: { label: "Cancel", minWidth: 96 },
		},
	]);
}

export default readonly(state);

registerResponseHandler(ResponseType.UpdateOpenDocumentsList, (responseData: Response) => {
	const documentListData = responseData as UpdateOpenDocumentsList;
	if (documentListData) {
		state.documents = documentListData.open_documents;
		state.title = state.documents[state.activeDocumentIndex];
	}
});
registerResponseHandler(ResponseType.SetActiveDocument, (responseData: Response) => {
	const documentData = responseData as SetActiveDocument;
	if (documentData) {
		state.activeDocumentIndex = documentData.document_index;
		state.title = state.documents[state.activeDocumentIndex];
	}
});
registerResponseHandler(ResponseType.DisplayConfirmationToCloseDocument, (responseData: Response) => {
	const data = responseData as DisplayConfirmationToCloseDocument;
	closeDocumentWithConfirmation(data.document_index);
});
registerResponseHandler(ResponseType.DisplayConfirmationToCloseAllDocuments, (_responseData: Response) => {
	closeAllDocumentsWithConfirmation();
});

(async () => (await wasm).get_open_documents_list())();
