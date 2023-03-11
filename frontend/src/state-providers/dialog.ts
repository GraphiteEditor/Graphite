import {writable} from "svelte/store";

import { type IconName } from "~/src/utility-functions/icons";
import { type Editor } from "~/src/wasm-communication/editor";
import { type TextButtonWidget, type WidgetLayout, defaultWidgetLayout, DisplayDialog, DisplayDialogDismiss, UpdateDialogDetails, patchWidgetLayout } from "~/src/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDialogState(editor: Editor) {
	const { subscribe, update } = writable({
		visible: false,
		icon: "" as IconName,
		widgets: defaultWidgetLayout(),
		// Special case for the crash dialog because we cannot handle button widget callbacks from Rust once the editor instance has panicked
		jsCallbackBasedButtons: undefined as undefined | TextButtonWidget[],
	});

	function dismissDialog(): void {
		update((state) => {
			state.visible = false;
			return state;
		});
	}

	// Creates a panic dialog from JS.
	// Normal dialogs are created in the Rust backend, but for the crash dialog, the editor instance has panicked so it cannot respond to widget callbacks.
	function createPanicDialog(icon: IconName, widgets: WidgetLayout, jsCallbackBasedButtons: TextButtonWidget[]): void {
		update((state) => {
			state.visible = true;
			state.icon = icon;
			state.widgets = widgets;
			state.jsCallbackBasedButtons = jsCallbackBasedButtons;
			return state;
		});
	}

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(DisplayDialog, (displayDialog) => {
		update((state) => {
			state.visible = true;
			state.icon = displayDialog.icon;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDialogDetails, (updateDialogDetails) => {
		update((state) => {
			patchWidgetLayout(state.widgets, updateDialogDetails);

			state.jsCallbackBasedButtons = undefined;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(DisplayDialogDismiss, dismissDialog);

	return {
		subscribe,
		dismissDialog,
		createPanicDialog,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
