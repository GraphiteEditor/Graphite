import { reactive, readonly } from "vue";

import { createDialog, dismissDialog } from "@/utilities/dialog";
import {
	ResponseType,
	registerResponseHandler,
	Response,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	SaveDocument,
} from "@/utilities/response-handler";
import { download, upload } from "@/utilities/files";
import wasm from "@/utilities/wasm-loader";

const state = reactive({
	title: "",
	unsaved: false,
	documents: [] as Array<string>,
	activeDocumentIndex: 0,
});

export async function selectDocument(tabIndex: number) {
	wasm().select_document(tabIndex);
}

export async function closeDocumentWithConfirmation(tabIndex: number) {
	selectDocument(tabIndex);

	const tabLabel = state.documents[tabIndex];

	createDialog("File", "Save changes before closing?", tabLabel, [
		{
			kind: "TextButton",
			callback: async () => {
				wasm().save_document();
				dismissDialog();
			},
			props: { label: "Save", emphasized: true, minWidth: 96 },
		},
		{
			kind: "TextButton",
			callback: async () => {
				wasm().close_document(tabIndex);
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
				wasm().close_all_documents();
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

registerResponseHandler(ResponseType.DisplayConfirmationToCloseAllDocuments, (_: Response) => {
	closeAllDocumentsWithConfirmation();
});

registerResponseHandler(ResponseType.OpenDocumentBrowse, async (_: Response) => {
	const extension = wasm().file_save_suffix();
	const data = await upload(extension);
	wasm().open_document_file(data.filename, data.content);
});

registerResponseHandler(ResponseType.ExportDocument, (responseData: Response) => {
	const updateData = responseData as ExportDocument;
	if (updateData) download(updateData.name, updateData.document);
});

registerResponseHandler(ResponseType.SaveDocument, (responseData: Response) => {
	const saveData = responseData as SaveDocument;
	if (saveData) download(saveData.name, saveData.document);
});
