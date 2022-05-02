import { reactive, readonly } from "vue";

import { defaultWidgetLayout, DisplayDialog, TriggerDismissDialog, UpdateDialogButtons, UpdateDialogDetails, Widget, WidgetLayout } from "@/dispatcher/js-messages";
import { EditorState } from "@/state/wasm-loader";
import { IconName } from "@/utilities/icons";
import { TextButtonWidget } from "@/utilities/widgets";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDialogState(editor: EditorState) {
	const state = reactive({
		visible: false,
		icon: "" as IconName,
		heading: "",
		details: defaultWidgetLayout(),
		buttons: defaultWidgetLayout(),
	});

	const createDialog = (icon: IconName, heading: string, details: string, buttons: TextButtonWidget[]): void => {
		const detailsLayout: WidgetLayout = {
			// eslint-disable-next-line camelcase
			layout: [{ widgets: [{ kind: "TextLabel", widget_id: BigInt(0), props: { value: details } }] }],
			// eslint-disable-next-line camelcase
			layout_target: null,
		};
		const buttonsLayout: WidgetLayout = {
			// eslint-disable-next-line camelcase
			layout: [
				{
					widgets: buttons.map((widget) => {
						const props = widget.props as any;
						props.action = widget.callback;
						return {
							kind: "TextButton",
							// eslint-disable-next-line camelcase
							widget_id: BigInt(0),
							props,
						};
					}),
				},
			],
			// eslint-disable-next-line camelcase
			layout_target: null,
		};

		state.visible = true;
		state.icon = icon;
		state.heading = heading;
		state.details = detailsLayout;
		state.buttons = buttonsLayout;
	};

	const dismissDialog = (): void => {
		state.visible = false;
	};

	const submitDialog = (): void => {
		const layout = state.buttons.layout[0] as { widgets: Widget[] };

		const firstEmphasizedButton = layout.widgets.find((button) => button.props.props.emphasized && button.props.callback);
		firstEmphasizedButton?.props.callback?.();
	};

	const dialogIsVisible = (): boolean => state.visible;

	// Run on creation
	editor.dispatcher.subscribeJsMessage(DisplayDialog, (displayDialog) => {
		state.heading = displayDialog.heading;
		state.icon = displayDialog.icon;
		state.visible = true;
	});

	editor.dispatcher.subscribeJsMessage(TriggerDismissDialog, dismissDialog);

	const comingSoon = (issueNumber?: number): void => {
		editor.instance.request_coming_soon_dialog(issueNumber);
	};

	editor.dispatcher.subscribeJsMessage(UpdateDialogDetails, (updateDialogDetails) => {
		state.details = updateDialogDetails;
	});

	editor.dispatcher.subscribeJsMessage(UpdateDialogButtons, (updateDialogButtons) => {
		state.buttons = updateDialogButtons;
	});

	return {
		state: readonly(state),
		createDialog,
		dismissDialog,
		submitDialog,
		dialogIsVisible,
		comingSoon,
	};
}
export type DialogState = ReturnType<typeof createDialogState>;
