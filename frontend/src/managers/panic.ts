import type { Editor } from "@graphite/editor";
import { createCrashDialog } from "@graphite/stores/dialog";

let editorRef: Editor | undefined = undefined;

export function createPanicManager(editor: Editor) {
	destroyPanicManager();

	editorRef = editor;

	editor.subscriptions.subscribeFrontendMessage("DisplayDialogPanic", (data) => {
		// `Error.stackTraceLimit` is only available in V8/Chromium
		const previousStackTraceLimit = Error.stackTraceLimit;
		Error.stackTraceLimit = Infinity;
		const stackTrace = new Error().stack || "";
		Error.stackTraceLimit = previousStackTraceLimit;

		const panicDetails = `${data.panicInfo}${stackTrace ? `\n\n${stackTrace}` : ""}`;

		// eslint-disable-next-line no-console
		console.error(panicDetails);

		createCrashDialog(panicDetails);
	});
}

export function destroyPanicManager() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("DisplayDialogPanic");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (editorRef) newModule?.createPanicManager(editorRef);
});
