/* eslint-disable import/no-cycle */
import { JsMessage } from "@/utilities/js-messages";
import { EditorState } from "@/state/wasm-loader";
import { DialogState } from "./dialog";
import { DocumentsState } from "./documents";
import { FullscreenState } from "./fullscreen";

interface AppState {
	dialog: DialogState;
	documents: DocumentsState;
	fullscreen: FullscreenState;
	editor: EditorState;
}

// Holds information across editor instances as well as registers global listeners
class GlobalEditorManager {
	private activeInstances = new Set<AppState>();

	constructor() {
		window.addEventListener("beforeunload", (e) => this.onBeforeUnload(e));
	}

	broadcastGlobalMessage(message: JsMessage) {
		[...this.activeInstances].forEach((instance) => {
			instance.editor.dispatcher.dispatchJsMessage(message);
		});
	}

	registerInstance(appState: AppState) {
		this.activeInstances.add(appState);
	}

	removeInstance(appState: AppState) {
		this.activeInstances.delete(appState);
	}

	onBeforeUnload(event: BeforeUnloadEvent) {
		const allDocumentsSaved = [...this.activeInstances].reduce((acc, instance) => instance.documents.state.documents.reduce((acc, doc) => doc.isSaved && acc, true) && acc, true);
		if (!allDocumentsSaved) {
			event.returnValue = "Unsaved work will be lost if the web browser tab is closed. Close anyway?";
			event.preventDefault();
		}
	}
}

export const globalEditorManager = new GlobalEditorManager();
