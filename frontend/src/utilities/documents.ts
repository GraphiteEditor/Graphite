import { reactive, readonly } from "vue";

import { createDialog, dismissDialog } from "@/utilities/dialog";
import {
	registerResponseHandler,
	DisplayConfirmationToCloseAllDocuments,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	SaveDocument,
	OpenDocumentBrowse,
} from "@/utilities/response-handler";
import { download, upload } from "@/utilities/files";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);

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

	createDialog("File", "Save changes before closing?", tabLabel, [
		{
			kind: "TextButton",
			callback: async () => {
				(await wasm).save_document();
				dismissDialog();
			},
			props: { label: "Save", emphasized: true, minWidth: 96 },
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

registerResponseHandler(UpdateOpenDocumentsList, (documentListData) => {
	if (documentListData) {
		state.documents = documentListData.open_documents;
		state.title = state.documents[state.activeDocumentIndex];
	}
});

registerResponseHandler(SetActiveDocument, (documentData) => {
	if (documentData) {
		state.activeDocumentIndex = documentData.document_index;
		state.title = state.documents[state.activeDocumentIndex];
	}
});

registerResponseHandler(DisplayConfirmationToCloseDocument, (data) => {
	closeDocumentWithConfirmation(data.document_index);
});

registerResponseHandler(DisplayConfirmationToCloseAllDocuments, (_) => {
	closeAllDocumentsWithConfirmation();
});

registerResponseHandler(OpenDocumentBrowse, async (_) => {
	const extension = (await wasm).file_save_suffix();
	const data = await upload(extension);
	(await wasm).open_document_file(data.filename, data.content);
});

registerResponseHandler(ExportDocument, (updateData) => {
	if (updateData) download(updateData.name, updateData.document);
});

registerResponseHandler(SaveDocument, (saveData) => {
	if (saveData) download(saveData.name, saveData.document);
});

(async () => (await wasm).get_open_documents_list())();
