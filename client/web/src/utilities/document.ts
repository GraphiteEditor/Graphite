import { reactive, readonly } from "vue";

const state = reactive({
	title: "",
	unsaved: false,
});

export function setDocumentTitle(title: string) {
	state.title = title;
}

export function setUnsavedState(isUnsaved: boolean) {
	state.unsaved = isUnsaved;
}

export function documentTitle(): string {
	return state.title;
}

export function documentIsUnsaved(): boolean {
	return state.unsaved;
}

export default readonly(state);
