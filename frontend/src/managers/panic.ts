import type { Editor } from "@graphite/editor";
import { createCrashDialog } from "@graphite/stores/dialog";

let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor] | undefined;

export function createPanicManager(editor: Editor) {
	currentArgs = [editor];
	// Code panic dialog and console error
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

	function destroy() {
		editor.subscriptions.unsubscribeFrontendMessage("DisplayDialogPanic");
	}

	currentCleanup = destroy;
	return { destroy };
}
export type PanicManager = ReturnType<typeof createPanicManager>;

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createPanicManager(...currentArgs);
});
