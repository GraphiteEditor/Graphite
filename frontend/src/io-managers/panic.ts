import type { Editor } from "@graphite/editor";
import type { DialogState } from "@graphite/state-providers/dialog";
import { browserVersion, operatingSystem } from "@graphite/utility-functions/platform";
import { stripIndents } from "@graphite/utility-functions/strip-indents";

let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor, DialogState] | undefined;

export function createPanicManager(editor: Editor, dialogState: DialogState): () => void {
	currentArgs = [editor, dialogState];
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

		dialogState.createCrashDialog(panicDetails);
	});

	currentCleanup = () => {
		editor.subscriptions.unsubscribeFrontendMessage("DisplayDialogPanic");
	};
	return currentCleanup;
}

export function githubUrl(panicDetails: string): string {
	const url = new URL("https://github.com/GraphiteEditor/Graphite/issues/new");

	const buildUrl = (includeCrashReport: boolean) => {
		let body = stripIndents`
			**Describe the Crash**
			Explain clearly what you were doing when the crash occurred.

			**Steps To Reproduce**
			Describe precisely how the crash occurred, step by step, starting with a new editor window.
			1. Open the Graphite editor at https://dev.graphite.art — IMPORTANT! Confirm you have tested in this development version. It may have already been fixed since the last stable release.
			2. 
			3. 
			4. 
			5. 

			**Additional Details**
			Provide any further information or context that you think would be helpful in fixing the issue. Screenshots or video can be linked or attached to this issue.

			**Browser and OS**
			${browserVersion()}, ${operatingSystem()}

			**Stack Trace**
			Copied from the crash dialog in the Graphite editor:
		`;

		const manualCopyStackTraceNotice = stripIndents`
			Before submitting this bug, REPLACE THIS WITH THE LOG. Return to the editor and click "Copy Error Log" in the crash dialog and paste it in place of this text.
		`;

		body += "\n\n```\n";
		body += includeCrashReport ? panicDetails.trimEnd() : manualCopyStackTraceNotice;
		body += "\n```";

		const fields = {
			title: "[Crash Report] ",
			body,
			labels: ["Crash"].join(","),
			projects: [].join(","),
			milestone: "",
			assignee: "",
			template: "",
		};

		Object.entries(fields).forEach(([field, value]) => {
			if (value) url.searchParams.set(field, value);
		});

		return String(url);
	};

	let urlString = buildUrl(true);
	if (urlString.length >= 8192) {
		// Fall back to a shorter version if it exceeds GitHub limits of 8192 total characters
		urlString = buildUrl(false);
	}
	return urlString;
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createPanicManager(...currentArgs);
});
