/* eslint-disable no-shadow */
import { reactive, readonly } from "vue";

import { TextButtonWidget } from "@/components/widgets/widgets";

export type DialogState = ReturnType<typeof makeDialogState>;

// For now, keep a reference to the last initialized state. This is still a global variable, but others depend on these functions.
let globalDialogState: DialogState | null;

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

	const result = {
		state: readonly(state),
		createDialog,
		dismissDialog,
		submitDialog,
		dialogIsVisible,
	};
	globalDialogState = result;
	return result;
}

// TODO: these need to go after all users have been migrated away from them
export function createDialog(icon: string, heading: string, details: string, buttons: TextButtonWidget[]) {
	if (!globalDialogState) throw new Error("no DialogState initialized");
	globalDialogState.createDialog(icon, heading, details, buttons);
}
export function dismissDialog() {
	if (!globalDialogState) throw new Error("no DialogState initialized");
	globalDialogState.dismissDialog();
}
export function submitDialog() {
	if (!globalDialogState) throw new Error("no DialogState initialized");
	globalDialogState.submitDialog();
}
export function dialogIsVisible() {
	if (!globalDialogState) throw new Error("no DialogState initialized");
	return globalDialogState.dialogIsVisible();
}
