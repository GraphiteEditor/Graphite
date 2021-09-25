/* eslint-disable no-shadow */
import { reactive, readonly } from "vue";

import { TextButtonWidget } from "@/components/widgets/widgets";

export type DialogState = ReturnType<typeof makeDialogState>;

export default function makeDialogState() {
	const state = reactive({
		visible: false,
		icon: "",
		heading: "",
		details: "",
		buttons: [] as TextButtonWidget[],
	});

	function createDialog(icon: string, heading: string, details: string, buttons: TextButtonWidget[]) {
		state.visible = true;
		state.icon = icon;
		state.heading = heading;
		state.details = details;
		state.buttons = buttons;
	}

	function dismissDialog() {
		state.visible = false;
	}

	function submitDialog() {
		const firstEmphasizedButton = state.buttons.find((button) => button.props.emphasized && button.callback);
		if (firstEmphasizedButton) {
			// If statement satisfies TypeScript
			if (firstEmphasizedButton.callback) firstEmphasizedButton.callback();
		}
	}

	function dialogIsVisible(): boolean {
		return state.visible;
	}

	function comingSoon(issueNumber?: number) {
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

	return {
		state: readonly(state),
		createDialog,
		dismissDialog,
		submitDialog,
		dialogIsVisible,
		comingSoon,
	};
}
