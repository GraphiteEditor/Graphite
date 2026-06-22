import { browserVersion, operatingSystem } from "/src/utility-functions/platform";
import { stripIndents } from "/src/utility-functions/strip-indents";

export function crashReportUrl(panicDetails: string): string {
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
