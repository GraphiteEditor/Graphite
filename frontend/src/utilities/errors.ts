import { createDialog, dismissDialog } from "@/utilities/dialog";
import { TextButtonWidget } from "@/components/widgets/widgets";
import { getPanicDetails } from "@/utilities/panic";
import { ResponseType, registerResponseHandler, Response, DisplayError, DisplayPanic } from "@/utilities/response-handler";

export function comingSoon(issueNumber?: number) {
	const bugMessage = `— but you can help add it!\nSee issue #${issueNumber} on GitHub.`;
	const details = `This feature is not implemented yet${issueNumber ? bugMessage : ""}`;

	const okButton: TextButtonWidget = {
		kind: "TextButton",
		callback: async () => dismissDialog(),
		props: { label: "OK", emphasized: true, minWidth: 96 },
	};
	const issueButton: TextButtonWidget = {
		kind: "TextButton",
		callback: async () => window.open(`https://github.com/GraphiteEditor/Graphite/issues/${issueNumber}`, "_blank"),
		props: { label: `Issue #${issueNumber}`, minWidth: 96 },
	};
	const buttons = [okButton];
	if (issueNumber) buttons.push(issueButton);

	createDialog("Warning", "Coming soon", details, buttons);
}

registerResponseHandler(ResponseType.DisplayError, (responseData: Response) => {
	const data = responseData as DisplayError;

	const okButton: TextButtonWidget = {
		kind: "TextButton",
		callback: async () => dismissDialog(),
		props: { label: "OK", emphasized: true, minWidth: 96 },
	};
	const buttons = [okButton];

	createDialog("Warning", data.title, data.description, buttons);
});

registerResponseHandler(ResponseType.DisplayPanic, (responseData: Response) => {
	const data = responseData as DisplayPanic;

	const reloadButton: TextButtonWidget = {
		kind: "TextButton",
		callback: async () => window.location.reload(),
		props: { label: "Reload", emphasized: true, minWidth: 96 },
	};
	const copyErrorLogButton: TextButtonWidget = {
		kind: "TextButton",
		callback: async () => navigator.clipboard.writeText(getPanicDetails()),
		props: { label: "Copy Error Log", emphasized: false, minWidth: 96 },
	};
	const reportOnGithubButton: TextButtonWidget = {
		kind: "TextButton",
		callback: async () => window.open(githubUrl(), "_blank"),
		props: { label: "Report Bug", emphasized: false, minWidth: 96 },
	};
	const buttons = [reloadButton, copyErrorLogButton, reportOnGithubButton];

	createDialog("Warning", data.title, data.description, buttons);
});

function githubUrl() {
	const url = new URL("https://github.com/GraphiteEditor/Graphite/issues/new");

	const body = `
**Describe the Crash**
Explain clearly what you were doing when the crash occurred.

**Steps To Reproduce**
Describe precisely how the crash occurred, step by step, starting with a new editor window.
1. Open the Graphite Editor at https://editor.graphite.design
2. 
3. 
4. 
5. 

**Browser and OS*
List of your browser and its version, as well as your operating system.

**Additional Details**
Provide any further information or context that you think would be helpful in fixing the issue. Screenshots or video can be linked or attached to this issue.

**Stack Trace**
Copied from the crash dialog in the Graphite Editor:

\`\`\`
${getPanicDetails()}
\`\`\`
`.trim();

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
