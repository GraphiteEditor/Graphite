import { reactive, readonly } from "vue";

import { DialogState } from "./dialog";
import { ResponseType, Response, SetActiveDocument, UpdateOpenDocumentsList, DisplayConfirmationToCloseDocument, ExportDocument, SaveDocument } from "@/state/response-handler";
import { download, upload } from "@/utilities/files";
import { EditorWasm } from "@/utilities/wasm-loader";

export type DocumentsState = ReturnType<typeof makeDocumentsState>;

export default function makeDocumentsState(editor: EditorWasm, dialogState: DialogState) {
	const state = reactive({
		title: "",
		unsaved: false,
		documents: [] as Array<string>,
		activeDocumentIndex: 0,
	});

	function selectDocument(tabIndex: number) {
		editor.select_document(tabIndex);
	}

	function closeDocumentWithConfirmation(tabIndex: number) {
		selectDocument(tabIndex);

		const tabLabel = state.documents[tabIndex];

		dialogState.createDialog("File", "Save changes before closing?", tabLabel, [
			{
				kind: "TextButton",
				callback: () => {
					editor.save_document();
					dialogState.dismissDialog();
				},
				props: { label: "Save", emphasized: true, minWidth: 96 },
			},
			{
				kind: "TextButton",
				callback: () => {
					editor.close_document(tabIndex);
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
					editor.close_all_documents();
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

	editor.registerResponseHandler(ResponseType.UpdateOpenDocumentsList, (responseData: Response) => {
		const documentListData = responseData as UpdateOpenDocumentsList;
		if (documentListData) {
			state.documents = documentListData.open_documents;
			state.title = state.documents[state.activeDocumentIndex];
		}
	});

	editor.registerResponseHandler(ResponseType.SetActiveDocument, (responseData: Response) => {
		const documentData = responseData as SetActiveDocument;
		if (documentData) {
			state.activeDocumentIndex = documentData.document_index;
			state.title = state.documents[state.activeDocumentIndex];
		}
	});

	editor.registerResponseHandler(ResponseType.DisplayConfirmationToCloseDocument, (responseData: Response) => {
		const data = responseData as DisplayConfirmationToCloseDocument;
		closeDocumentWithConfirmation(data.document_index);
	});

	editor.registerResponseHandler(ResponseType.DisplayConfirmationToCloseAllDocuments, (_: Response) => {
		closeAllDocumentsWithConfirmation();
	});

	editor.registerResponseHandler(ResponseType.OpenDocumentBrowse, async (_: Response) => {
		const extension = editor.file_save_suffix();
		const data = await upload(extension);
		editor.open_document_file(data.filename, data.content);
	});

	editor.registerResponseHandler(ResponseType.ExportDocument, (responseData: Response) => {
		const updateData = responseData as ExportDocument;
		if (updateData) download(updateData.name, updateData.document);
	});

	editor.registerResponseHandler(ResponseType.SaveDocument, (responseData: Response) => {
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
