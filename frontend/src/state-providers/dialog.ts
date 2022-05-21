import { reactive, readonly } from "vue";

import { TextButtonWidget } from "@/components/widgets/buttons/TextButton";
import { Editor } from "@/interop/editor";
import { defaultWidgetLayout, DisplayDialog, DisplayDialogDismiss, UpdateDialogDetails, WidgetLayout } from "@/interop/messages";
import { IconName } from "@/utilities/icons";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDialogState(editor: Editor) {
	const state = reactive({
		visible: false,
		icon: "" as IconName,
		widgets: defaultWidgetLayout(),
		// Special case for the crash dialog because we cannot handle button widget callbacks from Rust once the editor instance has panicked
		jsCallbackBasedButtons: undefined as undefined | TextButtonWidget[],
	});

	function dismissDialog(): void {
		state.visible = false;
	}

	function dialogIsVisible(): boolean {
		return state.visible;
	}

	// Creates a panic dialog from JS.
	// Normal dialogs are created in the Rust backend, but for the crash dialog, the editor instance has panicked so it cannot respond to widget callbacks.
	function createPanicDialog(icon: IconName, widgets: WidgetLayout, jsCallbackBasedButtons: TextButtonWidget[]): void {
		state.visible = true;
		state.icon = icon;
		state.widgets = widgets;
		state.jsCallbackBasedButtons = jsCallbackBasedButtons;
	}

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(DisplayDialog, (displayDialog) => {
		state.visible = true;
		state.icon = displayDialog.icon;
	});
	editor.subscriptions.subscribeJsMessage(UpdateDialogDetails, (updateDialogDetails) => {
		state.widgets = updateDialogDetails;
		state.jsCallbackBasedButtons = undefined;
	});
	editor.subscriptions.subscribeJsMessage(DisplayDialogDismiss, dismissDialog);

	return {
		state: readonly(state) as typeof state,
		dismissDialog,
		dialogIsVisible,
		createPanicDialog,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
