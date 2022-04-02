import { DisplayDialogError, DisplayDialogPanic } from "@/dispatcher/js-messages";
import { DialogState } from "@/state/dialog";
import { EditorState } from "@/state/wasm-loader";
import { stripIndents } from "@/utilities/strip-indents";
import { TextButtonWidget } from "@/utilities/widgets";

export function initErrorHandling(editor: EditorState, dialogState: DialogState): void {
	// Graphite error dialog
	editor.dispatcher.subscribeJsMessage(DisplayDialogError, (displayDialogError) => {
		const okButton: TextButtonWidget = {
			kind: "TextButton",
			callback: async () => dialogState.dismissDialog(),
			props: { label: "OK", emphasized: true, minWidth: 96 },
		};
		const buttons = [okButton];

		dialogState.createDialog("Warning", displayDialogError.title, displayDialogError.description, buttons);
	});

	// Code panic dialog and console error
	editor.dispatcher.subscribeJsMessage(DisplayDialogPanic, (displayDialogPanic) => {
		// `Error.stackTraceLimit` is only available in V8/Chromium
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		(Error as any).stackTraceLimit = Infinity;
		const stackTrace = new Error().stack || "";
		const panicDetails = `${displayDialogPanic.panic_info}\n\n${stackTrace}`;

		// eslint-disable-next-line no-console
		console.error(panicDetails);

		const reloadButton: TextButtonWidget = {
			kind: "TextButton",
			callback: async () => window.location.reload(),
			props: { label: "Reload", emphasized: true, minWidth: 96 },
		};
		const copyErrorLogButton: TextButtonWidget = {
			kind: "TextButton",
			callback: async () => navigator.clipboard.writeText(panicDetails),
			props: { label: "Copy Error Log", emphasized: false, minWidth: 96 },
		};
		const reportOnGithubButton: TextButtonWidget = {
			kind: "TextButton",
			callback: async () => window.open(githubUrl(panicDetails), "_blank"),
			props: { label: "Report Bug", emphasized: false, minWidth: 96 },
		};
		const buttons = [reloadButton, copyErrorLogButton, reportOnGithubButton];

		dialogState.createDialog("Warning", displayDialogPanic.title, displayDialogPanic.description, buttons);
	});
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
		${browserVersion()}, ${operatingSystem()}

		**Stack Trace**
		Copied from the crash dialog in the Graphite Editor:`;

	body += "\n\n```\n";
	body += panicDetails;
	body += "```";

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

function browserVersion(): string {
	const agent = window.navigator.userAgent;
	let match = agent.match(/(opera|chrome|safari|firefox|msie|trident(?=\/))\/?\s*(\d+)/i) || [];

	if (/trident/i.test(match[1])) {
		const browser = /\brv[ :]+(\d+)/g.exec(agent) || [];
		return `IE ${browser[1] || ""}`.trim();
	}

	if (match[1] === "Chrome") {
		let browser = agent.match(/\bEdg\/(\d+)/);
		if (browser !== null) return `Edge (Chromium) ${browser[1]}`;

		browser = agent.match(/\bOPR\/(\d+)/);
		if (browser !== null) return `Opera ${browser[1]}`;
	}

	match = match[2] ? [match[1], match[2]] : [navigator.appName, navigator.appVersion, "-?"];

	const browser = agent.match(/version\/(\d+)/i);
	if (browser !== null) match.splice(1, 1, browser[1]);

	return `${match[0]} ${match[1]}`;
}

function operatingSystem(): string {
	const osTable: Record<string, string> = {
		"Windows NT 10": "Windows 10 or 11",
		"Windows NT 6.3": "Windows 8.1",
		"Windows NT 6.2": "Windows 8",
		"Windows NT 6.1": "Windows 7",
		"Windows NT 6.0": "Windows Vista",
		"Windows NT 5.1": "Windows XP",
		"Windows NT 5.0": "Windows 2000",
		Mac: "Mac",
		X11: "Unix",
		Linux: "Linux",
		Unknown: "YOUR OPERATING SYSTEM",
	};

	const userAgentOS = Object.keys(osTable).find((key) => window.navigator.userAgent.includes(key));
	return osTable[userAgentOS || "Unknown"];
}
