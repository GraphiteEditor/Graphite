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
}
