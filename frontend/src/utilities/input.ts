import { toggleFullscreen } from "@/utilities/fullscreen";
import { dialogIsVisible, dismissDialog, submitDialog } from "@/utilities/dialog";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);

let viewportMouseInteractionOngoing = false;

// Keyboard and mouse events
type MouseKeys = "Lmb" | "Rmb" | "Mmb" | "Bmb" | "Fmb";
const keyboardKeys = {
	A: "KeyA",
	B: "KeyB",
	C: "KeyC",
	D: "KeyD",
	E: "KeyE",
	F: "KeyF",
	G: "KeyG",
	H: "KeyH",
	I: "KeyI",
	J: "KeyJ",
	K: "KeyK",
	L: "KeyL",
	M: "KeyM",
	N: "KeyN",
	O: "KeyO",
	P: "KeyP",
	Q: "KeyQ",
	R: "KeyR",
	S: "KeyS",
	T: "KeyT",
	U: "KeyU",
	V: "KeyV",
	W: "KeyW",
	X: "KeyX",
	Y: "KeyY",
	Z: "KeyZ",
	"0": "Key0",
	"1": "Key1",
	"2": "Key2",
	"3": "Key3",
	"4": "Key4",
	"5": "Key5",
	"6": "Key6",
	"7": "Key7",
	"8": "Key8",
	"9": "Key9",
	Enter: "KeyEnter",
	"=": "KeyEquals",
	"-": "KeyMinus",
	"+": "KeyPlus",
	Shift: "KeyShift",
	Space: "KeySpace",
	Control: "KeyControl",
	Delete: "KeyDelete",
	Backspace: "KeyBackspace",
	Alt: "KeyAlt",
	Escape: "KeyEscape",
	Tab: "KeyTab",
	ArrowUp: "KeyArrowUp",
	ArrowDown: "KeyArrowDown",
	ArrowLeft: "KeyArrowLeft",
	ArrowRight: "KeyArrowRight",
	LeftBracket: "KeyLeftBracket",
	RightBracket: "KeyRightBracket",
	LeftCurlyBracket: "KeyLeftCurlyBracket",
	RightCurlyBracket: "KeyRightCurlyBracket",
	PageUp: "KeyPageUp",
	PageDown: "KeyPageDown",
	Comma: "KeyComma",
	Period: "KeyPeriod",
};
type KeyboardKeys = typeof keyboardKeys[keyof typeof keyboardKeys];
type ModifierKeys = "ctrl" | "cmd" | "shift" | "alt";
type EventBehavior = "stop" | "prevent" | "self";
type HandlerChoice = { functionName: string; includeKeys: (MouseKeys | KeyboardKeys)[]; includeModifiers: ModifierKeys[]; excludeModifiers: ModifierKeys[]; eventBehavior: EventBehavior[] };
type Handlers = HandlerChoice[];

const standardKeymap: Record<string, Handlers> = {
	layerTreeLayerClick: [
		{ functionName: "handleControlClick", includeKeys: [], includeModifiers: ["ctrl"], excludeModifiers: ["shift", "alt"], eventBehavior: ["stop"] },
		{ functionName: "handleShiftClick", includeKeys: [], includeModifiers: ["shift"], excludeModifiers: ["ctrl", "alt"], eventBehavior: ["stop"] },
		{ functionName: "handleControlClick", includeKeys: [], includeModifiers: ["alt"], excludeModifiers: ["ctrl", "shift"], eventBehavior: ["stop"] },
		{ functionName: "handleClick", includeKeys: [], includeModifiers: [], excludeModifiers: ["ctrl", "shift", "alt"], eventBehavior: ["stop"] },
	],
	numberInputAbort: [{ functionName: "numberInputAbort", includeKeys: ["KeyEscape"], includeModifiers: [], excludeModifiers: [], eventBehavior: [] }],
};
const keymapApple: Record<string, Handlers> = {
	layerTreeLayerClick: [
		{ functionName: "handleControlClick", includeKeys: [], includeModifiers: ["cmd"], excludeModifiers: ["ctrl", "shift", "alt"], eventBehavior: ["stop"] },
		{ functionName: "handleShiftClick", includeKeys: [], includeModifiers: ["shift"], excludeModifiers: ["ctrl", "cmd", "alt"], eventBehavior: ["stop"] },
		{ functionName: "handleControlClick", includeKeys: [], includeModifiers: ["alt"], excludeModifiers: ["ctrl", "cmd", "shift"], eventBehavior: ["stop"] },
		{ functionName: "handleClick", includeKeys: [], includeModifiers: [], excludeModifiers: ["ctrl", "cmd", "shift", "alt"], eventBehavior: ["stop"] },
	],
	numberInputAbort: [{ functionName: "numberInputAbort", includeKeys: ["KeyEscape"], includeModifiers: [], excludeModifiers: [], eventBehavior: [] }],
};

export function handleInputEvent(event: KeyboardEvent | MouseEvent | TouchEvent, keymapEntryId: keyof typeof standardKeymap, functions: Record<string, () => void>) {
	const isApple = /^Mac|^iPhone|^iPad/i.test(navigator.platform);
	const keymap = isApple ? keymapApple : standardKeymap;
	const handlers = keymap[keymapEntryId];

	// Same physical key on all keyboard layouts but used differently between platform keymaps
	const ctrlModifier = event.ctrlKey;
	// Used only by the Apple keymap (Graphite should never use the meta/Windows key on the non-Apple platform keymap)
	const cmdModifier = isApple && event.metaKey;
	// Consistent across all keyboard layouts
	const shiftModifier = event.shiftKey;
	// Consistent across all keyboard layouts but this physical key is labeled "Option" on Apple layouts
	const altModifier = event.altKey;

	const matchedHandlers = handlers.filter((handler) => {
		// Reject any excluded modifier keys
		if (ctrlModifier && handler.excludeModifiers.includes("ctrl")) return false;
		if (cmdModifier && handler.excludeModifiers.includes("cmd")) return false;
		if (shiftModifier && handler.excludeModifiers.includes("shift")) return false;
		if (altModifier && handler.excludeModifiers.includes("alt")) return false;

		// Reject if missing any included modifier keys
		if (!ctrlModifier && handler.includeModifiers.includes("ctrl")) return false;
		if (!cmdModifier && handler.includeModifiers.includes("cmd")) return false;
		if (!shiftModifier && handler.includeModifiers.includes("shift")) return false;
		if (!altModifier && handler.includeModifiers.includes("alt")) return false;

		if (event instanceof MouseEvent) {
			// handler.includeKeys.includes(mouseKeys[event.buttons])
			// const key = mouseKeys?[event.button + ""];
			let lmb = false; // Left
			let rmb = false; // Right
			let mmb = false; // Middle
			let bmb = false; // Back
			let fmb = false; // Forward

			let buttonsValue = event.buttons;
			if (buttonsValue >= 16) {
				fmb = true;
				buttonsValue -= 16;
			}
			if (buttonsValue >= 8) {
				bmb = true;
				buttonsValue -= 8;
			}
			if (buttonsValue >= 4) {
				mmb = true;
				buttonsValue -= 4;
			}
			if (buttonsValue >= 2) {
				rmb = true;
				buttonsValue -= 2;
			}
			if (buttonsValue >= 1) {
				lmb = true;
				buttonsValue -= 1;
			}

			if (!lmb && handler.includeKeys.includes("Lmb")) return false;
			if (!rmb && handler.includeKeys.includes("Rmb")) return false;
			if (!mmb && handler.includeKeys.includes("Mmb")) return false;
			if (!bmb && handler.includeKeys.includes("Bmb")) return false;
			if (!fmb && handler.includeKeys.includes("Fmb")) return false;
		}

		if (event instanceof KeyboardEvent) {
			event.key;
		}

		// Reject unmatched keybind
		return false;
	});

	if (matchedHandlers.length === 0) return;
	if (matchedHandlers.length > 1) {
		console.log(`Ambiguous set of matched input event handlers.
Keymap entry ID: ${keymapEntryId}
Modifiers: [Ctrl: ${ctrlModifier}] [Cmd: ${cmdModifier}] [Shift: ${shiftModifier}] [Alt: ${altModifier}]
Matched handlers: ${matchedHandlers}
`);
	}
	const handler = matchedHandlers[0];

	// Apply any additional event behavior
	if (handler.eventBehavior.includes("stop")) event.stopPropagation();
	if (handler.eventBehavior.includes("prevent")) event.preventDefault();
	if (handler.eventBehavior.includes("self") && event.target !== event.currentTarget) return;

	// Execute the callback function
	functions[handler.functionName]();
}

// Keyboard events

function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent): boolean {
	// Don't redirect user input from text entry into HTML elements
	const target = e.target as HTMLElement;
	if (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable) return false;

	// Don't redirect when a modal is covering the workspace
	if (dialogIsVisible()) return false;

	// Don't redirect a fullscreen request
	if (e.key.toLowerCase() === "f11" && e.type === "keydown" && !e.repeat) {
		e.preventDefault();
		toggleFullscreen();
		return false;
	}

	// Don't redirect a reload request
	if (e.key.toLowerCase() === "f5") return false;

	// Don't redirect debugging tools
	if (e.key.toLowerCase() === "f12") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "c") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "i") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "j") return false;

	// Redirect to the backend
	return true;
}

export async function onKeyDown(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_key_down(e.key, modifiers);
		return;
	}

	if (dialogIsVisible()) {
		if (e.key === "Escape") dismissDialog();
		if (e.key === "Enter") {
			submitDialog();

			// Prevent the Enter key from acting like a click on the last clicked button, which might reopen the dialog
			e.preventDefault();
		}
	}
}

export async function onKeyUp(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_key_up(e.key, modifiers);
	}
}

// Mouse events

export async function onMouseMove(e: MouseEvent) {
	if (!e.buttons) viewportMouseInteractionOngoing = false;

	const modifiers = makeModifiersBitfield(e);
	(await wasm).on_mouse_move(e.clientX, e.clientY, e.buttons, modifiers);
}

export async function onMouseDown(e: MouseEvent) {
	const target = e.target && (e.target as HTMLElement);
	const inCanvas = target && target.closest(".canvas");
	const inDialog = target && target.closest(".dialog-modal .floating-menu-content");

	// Block middle mouse button auto-scroll mode
	if (e.button === 1) e.preventDefault();

	if (dialogIsVisible() && !inDialog) {
		dismissDialog();
		e.preventDefault();
		e.stopPropagation();
	}

	if (inCanvas) viewportMouseInteractionOngoing = true;

	if (viewportMouseInteractionOngoing) {
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_mouse_down(e.clientX, e.clientY, e.buttons, modifiers);
	}
}

export async function onMouseUp(e: MouseEvent) {
	if (!e.buttons) viewportMouseInteractionOngoing = false;

	const modifiers = makeModifiersBitfield(e);
	(await wasm).on_mouse_up(e.clientX, e.clientY, e.buttons, modifiers);
}

export async function onMouseScroll(e: WheelEvent) {
	const target = e.target && (e.target as HTMLElement);
	const inCanvas = target && target.closest(".canvas");

	if (inCanvas) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_mouse_scroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
	}
}

export async function onWindowResize() {
	const viewports = Array.from(document.querySelectorAll(".canvas"));
	const boundsOfViewports = viewports.map((canvas) => {
		const bounds = canvas.getBoundingClientRect();
		return [bounds.left, bounds.top, bounds.right, bounds.bottom];
	});

	const flattened = boundsOfViewports.flat();
	const data = Float64Array.from(flattened);

	if (boundsOfViewports.length > 0) (await wasm).bounds_of_viewports(data);
}

export function makeModifiersBitfield(e: MouseEvent | KeyboardEvent): number {
	return Number(e.ctrlKey) | (Number(e.shiftKey) << 1) | (Number(e.altKey) << 2);
}
