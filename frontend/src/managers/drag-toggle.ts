// Drag-toggle: the user presses one toggleable button and drags across its siblings to flip them all
// to the opposite of the source's starting state.
//
// Peers declare themselves with two data attributes on the clickable element:
// - data-drag-toggle-group="<group-name>": siblings share this name to form a drag-toggle group
// - data-drag-toggle-state="<current-state>": the current toggle state (e.g. "visible" / "hidden")
//
// The gesture only engages once the pointer crosses from the source into a different sibling, so plain
// clicks still toggle as usual. When engaged, the source is clicked once (toggling it) and any sibling
// the pointer enters whose state still matches the source's recorded starting state is also clicked.

export type DragToggleListener = {
	// Given the group of the pressed button, and undefined once the press ends
	activeGroupChanged?: (activeGroup: string | undefined) => void;
	// Told when the gesture engages, before any of its clicks, and again when it ends, so its clicks can form one history step
	engagedChanged?: (engaged: boolean) => void;
};

const listeners = new Set<DragToggleListener>();
let activeGroup: string | undefined = undefined;
let source: HTMLElement | undefined = undefined;
let startingState: string | undefined = undefined;
let visited = new WeakSet<Element>();
let engaged = false;
let suppressNextClickFromSource: HTMLElement | undefined = undefined;

export function createDragToggleManager(listener: DragToggleListener) {
	listeners.add(listener);

	// Install the window event listeners only when the first consumer subscribes
	if (listeners.size === 1) {
		// Capture phase on pointerdown preempts sibling drag handlers on ancestors so they don't also engage
		window.addEventListener("pointerdown", onPointerDown, true);
		window.addEventListener("pointermove", onPointerMove);
		window.addEventListener("pointerup", onPointerUp);
		// A press the browser cancels, or loses to another window, ends like a release
		window.addEventListener("pointercancel", onPointerUp);
		window.addEventListener("blur", onPointerUp);
		// Capture phase on click suppresses the natural source-click before the button's handler runs
		window.addEventListener("click", onClickCapture, true);
	}
}

export function destroyDragToggleManager(listener: DragToggleListener) {
	// A consumer leaving mid-gesture still hears it end
	if (engaged) listener.engagedChanged?.(false);
	listeners.delete(listener);

	// Uninstall the window event listeners only once the last consumer leaves
	if (listeners.size === 0) {
		window.removeEventListener("pointerdown", onPointerDown, true);
		window.removeEventListener("pointermove", onPointerMove);
		window.removeEventListener("pointerup", onPointerUp);
		window.removeEventListener("pointercancel", onPointerUp);
		window.removeEventListener("blur", onPointerUp);
		window.removeEventListener("click", onClickCapture, true);

		activeGroup = undefined;
		source = undefined;
		startingState = undefined;
		visited = new WeakSet();
		engaged = false;
		suppressNextClickFromSource = undefined;
	}
}

function notifyActiveGroupChange(group: string | undefined) {
	activeGroup = group;
	listeners.forEach((listener) => listener.activeGroupChanged?.(group));
}

function findMember(target: EventTarget | undefined): HTMLElement | undefined {
	if (!(target instanceof Element)) return undefined;
	const found = target.closest("[data-drag-toggle-group]");
	return found instanceof HTMLElement ? found : undefined;
}

function onPointerDown(e: PointerEvent) {
	if (e.button !== 0) return;
	suppressNextClickFromSource = undefined;

	const found = findMember(e.target || undefined);
	if (!found) return;

	// Stop the event so sibling drag/select handlers on ancestors don't also engage
	e.stopPropagation();

	source = found;
	startingState = found.getAttribute("data-drag-toggle-state") || undefined;
	visited = new WeakSet();
	engaged = false;

	notifyActiveGroupChange(found.getAttribute("data-drag-toggle-group") || undefined);
}

function onPointerMove(e: PointerEvent) {
	if (!activeGroup || !source) return;

	const member = findMember(e.target || undefined);
	if (!member || member.getAttribute("data-drag-toggle-group") !== activeGroup) return;

	// Engages only when the cursor crosses from the source to a different peer, so tiny wobbles over the source don't trigger
	if (member === source || visited.has(member)) return;

	// First crossing engages the drag and toggles the source as part of the operation
	if (!engaged) {
		engaged = true;
		visited.add(source);
		listeners.forEach((listener) => listener.engagedChanged?.(true));
		if ((source.getAttribute("data-drag-toggle-state") || undefined) === startingState) source.click();
	}

	// Toggle only peers still in the starting state, so we don't flip ones already at the target state
	if ((member.getAttribute("data-drag-toggle-state") || undefined) !== startingState) return;

	visited.add(member);
	member.click();
}

function onPointerUp(e: Event) {
	if (!activeGroup) return;

	// An engaged drag already clicked the source, so the click its release sends is suppressed rather than toggling it again
	if (engaged && source && e.type === "pointerup") suppressNextClickFromSource = source;
	const wasEngaged = engaged;

	source = undefined;
	startingState = undefined;
	visited = new WeakSet();
	engaged = false;

	notifyActiveGroupChange(undefined);
	if (wasEngaged) listeners.forEach((listener) => listener.engagedChanged?.(false));
}

function onClickCapture(e: Event) {
	if (suppressNextClickFromSource && e.target instanceof Node && suppressNextClickFromSource.contains(e.target)) {
		e.stopPropagation();
		e.preventDefault();
	}
	suppressNextClickFromSource = undefined;
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	const carried = Array.from(listeners);
	carried.forEach((listener) => destroyDragToggleManager(listener));
	carried.forEach((listener) => newModule?.createDragToggleManager(listener));
});
