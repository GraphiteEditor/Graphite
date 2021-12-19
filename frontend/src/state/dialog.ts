import { reactive, readonly } from "vue";

import { TextButtonWidget } from "@/components/widgets/widgets";
import { EditorState } from "@/state/wasm-loader";
import { DisplayAboutGraphiteDialog } from "@/dispatcher/js-messages";
import { stripIndents } from "@/utilities/strip-indents";

export type DialogState = ReturnType<typeof createDialogState>;
export function createDialogState(editor: EditorState) {
	const state = reactive({
		visible: false,
		icon: "",
		heading: "",
		details: "",
		buttons: [] as TextButtonWidget[],
	});

	const createDialog = (icon: string, heading: string, details: string, buttons: TextButtonWidget[]) => {
		state.visible = true;
		state.icon = icon;
		state.heading = heading;
		state.details = details;
		state.buttons = buttons;
	};

	const dismissDialog = () => {
		state.visible = false;
	};

	const submitDialog = () => {
		const firstEmphasizedButton = state.buttons.find((button) => button.props.emphasized && button.callback);
		if (firstEmphasizedButton) {
			// If statement satisfies TypeScript
			if (firstEmphasizedButton.callback) firstEmphasizedButton.callback();
		}
	};

	const dialogIsVisible = (): boolean => {
		return state.visible;
	};

	const comingSoon = (issueNumber?: number) => {
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
	};

	const onAboutHandler = () => {
		const date = new Date(process.env.VUE_APP_COMMIT_DATE || "");
		const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
		const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
		const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "long" })
			.formatToParts(new Date())
			.find((part) => part.type === "timeZoneName");
		const timezoneNameString = timezoneName && timezoneName.value;

		const hash = (process.env.VUE_APP_COMMIT_HASH || "").substring(0, 12);

		const details = stripIndents`
			Release Series: ${process.env.VUE_APP_RELEASE_SERIES}

			Date: ${dateString} ${timeString} ${timezoneNameString}
			Hash: ${hash}
			Branch: ${process.env.VUE_APP_COMMIT_BRANCH}
			`;

		const buttons: TextButtonWidget[] = [
			{
				kind: "TextButton",
				callback: () => window.open("https://www.graphite.design", "_blank"),
				props: { label: "Website", emphasized: false, minWidth: 0 },
			},
			{
				kind: "TextButton",
				callback: () => window.open("https://github.com/GraphiteEditor/Graphite/graphs/contributors", "_blank"),
				props: { label: "Credits", emphasized: false, minWidth: 0 },
			},
			{
				kind: "TextButton",
				callback: () => window.open("https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/LICENSE.txt", "_blank"),
				props: { label: "License", emphasized: false, minWidth: 0 },
			},
			{
				kind: "TextButton",
				callback: () => window.open("/third-party-licenses.txt", "_blank"),
				props: { label: "Third-Party Licenses", emphasized: false, minWidth: 0 },
			},
		];

		createDialog("GraphiteLogo", "Graphite", details, buttons);
	};

	// Run on creation
	editor.dispatcher.subscribeJsMessage(DisplayAboutGraphiteDialog, () => onAboutHandler());

	return {
		state: readonly(state),
		createDialog,
		dismissDialog,
		submitDialog,
		dialogIsVisible,
		comingSoon,
	};
}
