/* eslint-disable no-shadow */
import { reactive } from "vue";

import { TextButtonWidget } from "@/components/widgets/widgets";

export class DialogState {
	private state = reactive({
		visible: false,
		icon: "",
		heading: "",
		details: "",
		buttons: [] as TextButtonWidget[],
	});

	createDialog(icon: string, heading: string, details: string, buttons: TextButtonWidget[]) {
		this.state.visible = true;
		this.state.icon = icon;
		this.state.heading = heading;
		this.state.details = details;
		this.state.buttons = buttons;
	}

	dismissDialog() {
		this.state.visible = false;
	}

	submitDialog() {
		const firstEmphasizedButton = this.state.buttons.find((button) => button.props.emphasized && button.callback);
		if (firstEmphasizedButton) {
			// If statement satisfies TypeScript
			if (firstEmphasizedButton.callback) firstEmphasizedButton.callback();
		}
	}

	dialogIsVisible(): boolean {
		return this.state.visible;
	}

	comingSoon(issueNumber?: number) {
		const bugMessage = `— but you can help add it!\nSee issue #${issueNumber} on GitHub.`;
		const details = `This feature is not implemented yet${issueNumber ? bugMessage : ""}`;

		const okButton: TextButtonWidget = {
			kind: "TextButton",
			callback: async () => this.dismissDialog(),
			props: { label: "OK", emphasized: true, minWidth: 96 },
		};
		const issueButton: TextButtonWidget = {
			kind: "TextButton",
			callback: async () => window.open(`https://github.com/GraphiteEditor/Graphite/issues/${issueNumber}`, "_blank"),
			props: { label: `Issue #${issueNumber}`, minWidth: 96 },
		};
		const buttons = [okButton];
		if (issueNumber) buttons.push(issueButton);

		this.createDialog("Warning", "Coming soon", details, buttons);
	}

	onAboutHandler() {
		const date = new Date(process.env.VUE_APP_COMMIT_DATE || "");
		const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
		const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
		const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "long" })
			.formatToParts(new Date())
			.find((part) => part.type === "timeZoneName");
		const timezoneNameString = timezoneName && timezoneName.value;

		const hash = (process.env.VUE_APP_COMMIT_HASH || "").substring(0, 12);

		const details = `
Release Series: ${process.env.VUE_APP_RELEASE_SERIES}

Date: ${dateString} ${timeString} ${timezoneNameString}
Hash: ${hash}
Branch: ${process.env.VUE_APP_COMMIT_BRANCH}
	`.trim();

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

		this.createDialog("GraphiteLogo", "Graphite", details, buttons);
	}
}
