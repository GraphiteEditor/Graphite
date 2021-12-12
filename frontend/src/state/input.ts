import { DialogState } from "@/state/dialog";
import { FullscreenState } from "@/state/fullscreen";
import { EditorState } from "./wasm-loader";

let viewportMouseInteractionOngoing = false;

type EventName = keyof HTMLElementEventMap;
interface EventListenerTarget {
	addEventListener: typeof window.addEventListener;
	removeEventListener: typeof window.removeEventListener;
}
export class InputManager {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	private listeners: { target: EventListenerTarget; eventName: EventName; action: (event: any) => void; options?: boolean | AddEventListenerOptions }[] = [
		{ target: window, eventName: "resize", action: () => this.onWindowResize(this.container) },
		{ target: window, eventName: "mousemove", action: (e) => this.onMouseMove(e) },
		{ target: this.container, eventName: "contextmenu", action: (e) => e.preventDefault() },
		{ target: this.container, eventName: "keyup", action: (e) => this.onKeyUp(e) },
		{ target: this.container, eventName: "keydown", action: (e) => this.onKeyDown(e) },
		{ target: this.container, eventName: "mousedown", action: (e) => this.onMouseDown(e) },
		{ target: this.container, eventName: "mouseup", action: (e) => this.onMouseUp(e) },
		{ target: this.container, eventName: "wheel", action: (e) => this.onMouseScroll(e), options: { passive: true } },
	];

	constructor(private container: HTMLElement, private fullscreen: FullscreenState, private dialog: DialogState, private editor: EditorState) {
		this.listeners.forEach(({ target, eventName, action, options }) => target.addEventListener(eventName, action, options));
		this.onWindowResize(container);
	}

	public removeListeners() {
		this.listeners.forEach(({ target, eventName, action }) => target.removeEventListener(eventName, action));
	}

	private shouldRedirectKeyboardEventToBackend(e: KeyboardEvent): boolean {
		// Don't redirect user input from text entry into HTML elements
		const target = e.target as HTMLElement;
		if (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable) return false;

		// Don't redirect when a modal is covering the workspace
		if (this.dialog.dialogIsVisible()) return false;

		// Don't redirect a fullscreen request
		if (e.key.toLowerCase() === "f11" && e.type === "keydown" && !e.repeat) {
			e.preventDefault();
			this.fullscreen.toggleFullscreen();
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

	private onKeyDown(e: KeyboardEvent) {
		if (this.shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeModifiersBitfield(e);
			this.editor.instance.on_key_down(e.key, modifiers);
			return;
		}

		if (this.dialog.dialogIsVisible()) {
			if (e.key === "Escape") this.dialog.dismissDialog();
			if (e.key === "Enter") {
				this.dialog.submitDialog();

				// Prevent the Enter key from acting like a click on the last clicked button, which might reopen the dialog
				e.preventDefault();
			}
		}
	}

	private onKeyUp(e: KeyboardEvent) {
		if (this.shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeModifiersBitfield(e);
			this.editor.instance.on_key_up(e.key, modifiers);
		}
	}

	private onMouseMove(e: MouseEvent) {
		if (!e.buttons) viewportMouseInteractionOngoing = false;

		const modifiers = makeModifiersBitfield(e);
		this.editor.instance.on_mouse_move(e.clientX, e.clientY, e.buttons, modifiers);
	}

	private onMouseDown(e: MouseEvent) {
		const target = e.target && (e.target as HTMLElement);
		const inCanvas = target && target.closest(".canvas");
		const inDialog = target && target.closest(".dialog-modal .floating-menu-content");

		// Block middle mouse button auto-scroll mode
		if (e.button === 1) e.preventDefault();

		if (this.dialog.dialogIsVisible() && !inDialog) {
			this.dialog.dismissDialog();
			e.preventDefault();
			e.stopPropagation();
		}

		if (inCanvas) viewportMouseInteractionOngoing = true;

		if (viewportMouseInteractionOngoing) {
			const modifiers = makeModifiersBitfield(e);
			this.editor.instance.on_mouse_down(e.clientX, e.clientY, e.buttons, modifiers);
		}
	}

	private onMouseUp(e: MouseEvent) {
		if (!e.buttons) viewportMouseInteractionOngoing = false;

		const modifiers = makeModifiersBitfield(e);
		this.editor.instance.on_mouse_up(e.clientX, e.clientY, e.buttons, modifiers);
	}

	private onMouseScroll(e: WheelEvent) {
		const target = e.target && (e.target as HTMLElement);
		const inCanvas = target && target.closest(".canvas");

		const horizontalScrollableElement = e.target instanceof Element && e.target.closest(".scrollable-x");
		if (horizontalScrollableElement && e.deltaY !== 0) {
			horizontalScrollableElement.scrollTo(horizontalScrollableElement.scrollLeft + e.deltaY, 0);
			return;
		}

		if (inCanvas) {
			e.preventDefault();
			const modifiers = makeModifiersBitfield(e);
			this.editor.instance.on_mouse_scroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
		}
	}

	onWindowResize(container: Element) {
		const viewports = Array.from(container.querySelectorAll(".canvas"));
		const boundsOfViewports = viewports.map((canvas) => {
			const bounds = canvas.getBoundingClientRect();
			return [bounds.left, bounds.top, bounds.right, bounds.bottom];
		});

		const flattened = boundsOfViewports.flat();
		const data = Float64Array.from(flattened);

		if (boundsOfViewports.length > 0) this.editor.instance.bounds_of_viewports(data);
	}
}

export function makeModifiersBitfield(e: MouseEvent | KeyboardEvent): number {
	return Number(e.ctrlKey) | (Number(e.shiftKey) << 1) | (Number(e.altKey) << 2);
}
