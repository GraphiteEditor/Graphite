import { reactive, readonly } from "vue";

import { DialogState } from "./dialog";
import {
	ResponseType,
	registerResponseHandler,
	Response,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	DisplayConfirmationToCloseDocument,
	ExportDocument,
	SaveDocument,
} from "@/state/response-handler";
import { download, upload } from "@/utilities/files";
import wasm from "@/utilities/wasm-loader";

export type DocumentsState = ReturnType<typeof makeDocumentsState>;

export default function makeDocumentsState(dialogState: DialogState) {
	const state = reactive({
		title: "",
		unsaved: false,
		documents: [] as Array<string>,
		activeDocumentIndex: 0,
	});

	function selectDocument(tabIndex: number) {
		wasm().select_document(tabIndex);
	}

	function closeDocumentWithConfirmation(tabIndex: number) {
		selectDocument(tabIndex);

		const tabLabel = state.documents[tabIndex];

		dialogState.createDialog("File", "Save changes before closing?", tabLabel, [
			{
				kind: "TextButton",
				callback: () => {
					wasm().save_document();
					dialogState.dismissDialog();
				},
				props: { label: "Save", emphasized: true, minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					wasm().close_document(tabIndex);
					dialogState.dismissDialog();
				},
				props: { label: "Discard", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	}

	function closeAllDocumentsWithConfirmation() {
		dialogState.createDialog("Copy", "Close all documents?", "Unsaved work will be lost!", [
			{
				kind: "TextButton",
				callback: () => {
					wasm().close_all_documents();
					dialogState.dismissDialog();
				},
				props: { label: "Discard All", minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					dialogState.dismissDialog();
				},
				props: { label: "Cancel", minWidth: 96 },
			},
		]);
	}

	// TODO: these use the global responseHandler instance.
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

	return {
		state: readonly(state),
		selectDocument,
		closeDocumentWithConfirmation,
		closeAllDocumentsWithConfirmation,
	};
}
