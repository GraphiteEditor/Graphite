import { reactive, readonly } from "vue";

import { defaultWidgetLayout, DisplayDialog, DisplayDialogDismiss, UpdateDialogDetails, WidgetLayout } from "@/dispatcher/js-messages";
import { EditorState } from "@/state/wasm-loader";
import { IconName } from "@/utilities/icons";
import { TextButtonWidget } from "@/utilities/widgets";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDialogState(editor: EditorState) {
	const state = reactive({
		visible: false,
		icon: "" as IconName,
		heading: "",
		widgets: defaultWidgetLayout() as WidgetLayout | undefined,
		/// Necessary because we cannot handle widget callbacks from Rust once the editor instance has panicked.
		jsComponents: undefined as { details: string; buttons: TextButtonWidget[] } | undefined,
	});

	/// Creates a dialog from JS
	/// Most dialogs should be done through Rust, however for the crash dialog, the editor instance has panicked so it cannot respond to widget callbacks.
	const createDialog = (icon: IconName, heading: string, details: string, buttons: TextButtonWidget[]): void => {
		state.visible = true;
		state.icon = icon;
		state.heading = heading;
		state.widgets = undefined;
		state.jsComponents = { details, buttons };
	};

	const dismissDialog = (): void => {
		state.visible = false;
	};

	const dialogIsVisible = (): boolean => state.visible;

	// Run on creation
	editor.dispatcher.subscribeJsMessage(DisplayDialog, (displayDialog) => {
		state.heading = displayDialog.heading;
		state.icon = displayDialog.icon;
		state.visible = true;
	});

	editor.dispatcher.subscribeJsMessage(DisplayDialogDismiss, dismissDialog);

	const comingSoon = (issueNumber?: number): void => {
		editor.instance.request_coming_soon_dialog(issueNumber);
	};

	editor.dispatcher.subscribeJsMessage(UpdateDialogDetails, (updateDialogDetails) => {
		state.widgets = updateDialogDetails;
		state.jsComponents = undefined;
	});

	return {
		state: readonly(state),
		createDialog,
		dismissDialog,
		dialogIsVisible,
		comingSoon,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
