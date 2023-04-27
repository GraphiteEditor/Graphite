import {writable} from "svelte/store";

import { type IconName } from "@graphite/utility-functions/icons";
import { type Editor } from "@graphite/wasm-communication/editor";
import { type TextButtonWidget, type WidgetLayout, defaultWidgetLayout, DisplayDialog, DisplayDialogDismiss, UpdateDialogDetails, patchWidgetLayout } from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDialogState(editor: Editor) {
	const { subscribe, update } = writable({
		visible: false,
		icon: "" as IconName,
		widgets: defaultWidgetLayout(),
		// Special case for the crash dialog because we cannot handle button widget callbacks from Rust once the editor instance has panicked
		crashDialogButtons: undefined as undefined | TextButtonWidget[],
	});

	function dismissDialog(): void {
		
		update((state) => {
			// Disallow dismissing the crash dialog since it can confuse users why the app stopped responding if they dismiss it without realizing what it means
			if (!state.crashDialogButtons) state.visible = false;
			
			return state;
		});
	}

	// Creates a crash dialog from JS once the editor has panicked.
	// Normal dialogs are created in the Rust backend, but for the crash dialog, the editor instance has panicked so it cannot respond to widget callbacks.
	function createCrashDialog(icon: IconName, widgets: WidgetLayout, crashDialogButtons: TextButtonWidget[]): void {
		update((state) => {
			state.visible = true;
			state.icon = icon;
			state.widgets = widgets;
			state.crashDialogButtons = crashDialogButtons;
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

			state.crashDialogButtons = undefined;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(DisplayDialogDismiss, dismissDialog);

	return {
		subscribe,
		dismissDialog,
		createCrashDialog: createCrashDialog,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
