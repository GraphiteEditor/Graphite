import { reactive, readonly } from "vue";

import { TextButtonWidget } from "@/components/widgets/widgets";

const state = reactive({
	visible: false,
	icon: "",
	heading: "",
	details: "",
	buttons: [] as TextButtonWidget[],
});

export function createDialog(icon: string, heading: string, details: string, buttons: TextButtonWidget[]) {
	state.visible = true;
	state.icon = icon;
	state.heading = heading;
	state.details = details;
	state.buttons = buttons;
}

export function dismissDialog() {
	state.visible = false;
}

export function dialogIsVisible(): boolean {
	return state.visible;
}

export default readonly(state);
