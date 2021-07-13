const wasm = import("@/../wasm/pkg");

export function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent): boolean {
	// Don't redirect user input from text entry into HTML elements
	const target = e.target as HTMLElement;
	if (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable) return false;

	// Don't redirect a fullscreen request
	if (e.key.toLowerCase() === "f11") return false;

	// Don't redirect debugging tools
	if (e.key.toLowerCase() === "f12") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "c") return false;

	// Redirect to the backend
	return true;
}

export async function handleKeyDown(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const { on_key_down } = await wasm;
		const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
		on_key_down(e.key, modifiers);
	}
}

export async function handleKeyUp(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const { on_key_up } = await wasm;
		const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
		on_key_up(e.key, modifiers);
	}
}

export function makeModifiersBitfield(control: boolean, shift: boolean, alt: boolean): number {
	// eslint-disable-next-line no-bitwise
	return Number(control) | (Number(shift) << 1) | (Number(alt) << 2);
}
