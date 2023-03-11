import { wipeDocuments } from "~/src/io-managers/persistence";
import { type DialogState } from "~/src/state-providers/dialog";
import { type IconName } from "~/src/utility-functions/icons";
import { browserVersion, operatingSystem } from "~/src/utility-functions/platform";
import { stripIndents } from "~/src/utility-functions/strip-indents";
import { type Editor } from "~/src/wasm-communication/editor";
import type { TextLabel } from "~/src/wasm-communication/messages";
import { type TextButtonWidget, type WidgetLayout, Widget, DisplayDialogPanic } from "~/src/wasm-communication/messages";

export function createPanicManager(editor: Editor, dialogState: DialogState): void {
	// Code panic dialog and console error
	editor.subscriptions.subscribeJsMessage(DisplayDialogPanic, (displayDialogPanic) => {
		// `Error.stackTraceLimit` is only available in V8/Chromium
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		(Error as any).stackTraceLimit = Infinity;
		const stackTrace = new Error().stack || "";
		const panicDetails = `${displayDialogPanic.panicInfo}${stackTrace ? `\n\n${stackTrace}` : ""}`;

		// eslint-disable-next-line no-console
		console.error(panicDetails);

		const panicDialog = preparePanicDialog(displayDialogPanic.header, displayDialogPanic.description, panicDetails);
		dialogState.createPanicDialog(...panicDialog);
	});
}

function preparePanicDialog(header: string, details: string, panicDetails: string): [IconName, WidgetLayout, TextButtonWidget[]] {
	const headerLabel: TextLabel = { kind: "TextLabel", value: header, disabled: false, bold: true, italic: false, tableAlign: false, minWidth: 0, multiline: false, tooltip: "" };
	const detailsLabel: TextLabel = { kind: "TextLabel", value: details, disabled: false, bold: false, italic: false, tableAlign: false, minWidth: 0, multiline: true, tooltip: "" };

	const widgets: WidgetLayout = {
		layout: [{ rowWidgets: [new Widget(headerLabel, 0n)] }, { rowWidgets: [new Widget(detailsLabel, 1n)] }],
		layoutTarget: undefined,
	};

	const reloadButton: TextButtonWidget = {
		callback: async () => window.location.reload(),
		props: { kind: "TextButton", label: "Reload", emphasized: true, minWidth: 96 },
	};
	const copyErrorLogButton: TextButtonWidget = {
		callback: async () => navigator.clipboard.writeText(panicDetails),
		props: { kind: "TextButton", label: "Copy Error Log", emphasized: false, minWidth: 96 },
	};
	const reportOnGithubButton: TextButtonWidget = {
		callback: async () => window.open(githubUrl(panicDetails), "_blank"),
		props: { kind: "TextButton", label: "Report Bug", emphasized: false, minWidth: 96 },
	};
	const clearPersistedDataButton: TextButtonWidget = {
		callback: async () => {
			await wipeDocuments();
			window.location.reload();
		},
		props: { kind: "TextButton", label: "Clear Saved Data", emphasized: false, minWidth: 96 },
	};
	const jsCallbackBasedButtons = [reloadButton, copyErrorLogButton, reportOnGithubButton, clearPersistedDataButton];

	return ["Warning", widgets, jsCallbackBasedButtons];
}

function githubUrl(panicDetails: string): string {
	const url = new URL("https://github.com/GraphiteEditor/Graphite/issues/new");

	let body = stripIndents`
		**Describe the Crash**
		Explain clearly what you were doing when the crash occurred.

		**Steps To Reproduce**
		Describe precisely how the crash occurred, step by step, starting with a new editor window.
		1. Open the Graphite Editor at https://editor.graphite.rs
		2. 
		3. 
		4. 
		5. 

		**Additional Details**
		Provide any further information or context that you think would be helpful in fixing the issue. Screenshots or video can be linked or attached to this issue.

		**Browser and OS**
		${browserVersion()}, ${operatingSystem(true).replace("Unknown", "YOUR OPERATING SYSTEM")}

		**Stack Trace**
		Copied from the crash dialog in the Graphite Editor:
	`;

	body += "\n\n```\n";
	body += panicDetails.trimEnd();
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

	return url.toString();
}
