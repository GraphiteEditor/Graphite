import { reactive, readonly } from "vue";

import { Editor } from "@/interop/editor";
import { defaultWidgetLayout, DisplayDialog, DisplayDialogDismiss, UpdateDialogDetails, WidgetLayout } from "@/interop/messages";
import { IconName } from "@/utilities/icons";
import { TextButtonWidget } from "@/utilities/widgets";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDialogState(editor: Editor) {
	const state = reactive({
		visible: false,
		icon: "" as IconName,
		widgets: defaultWidgetLayout(),
		// Special case for the crash dialog because we cannot handle button widget callbacks from Rust once the editor instance has panicked
		jsCallbackBasedButtons: undefined as undefined | TextButtonWidget[],
	});

	// Creates a panic dialog from JS.
	// Normal dialogs are created in the Rust backend, however for the crash dialog, the editor instance has panicked so it cannot respond to widget callbacks.
	const createPanicDialog = (widgets: WidgetLayout, jsCallbackBasedButtons: TextButtonWidget[]): void => {
		state.visible = true;
		state.icon = "Warning";
		state.widgets = widgets;
		state.jsCallbackBasedButtons = jsCallbackBasedButtons;
	};

	const dismissDialog = (): void => {
		state.visible = false;
	};

	const dialogIsVisible = (): boolean => state.visible;

	const comingSoon = (issueNumber?: number): void => {
		editor.instance.request_coming_soon_dialog(issueNumber);
	};

	// Run on creation
	editor.subscriptions.subscribeJsMessage(DisplayDialog, (displayDialog) => {
		state.visible = true;
		state.icon = displayDialog.icon;
	});

	editor.subscriptions.subscribeJsMessage(DisplayDialogDismiss, dismissDialog);

	editor.subscriptions.subscribeJsMessage(UpdateDialogDetails, (updateDialogDetails) => {
		state.widgets = updateDialogDetails;
		state.jsCallbackBasedButtons = undefined;
	});

	return {
		state: readonly(state),
		createPanicDialog,
		dismissDialog,
		dialogIsVisible,
		comingSoon,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
