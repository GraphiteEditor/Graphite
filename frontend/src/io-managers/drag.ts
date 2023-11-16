let draggingElement: HTMLElement | undefined;

export function createDragManager(): () => void {
	const clearDraggingElement = () => {
		draggingElement = undefined;
	};

	// Add the event listener
	document.addEventListener("drop", clearDraggingElement);

	// Return the destructor
	return () => {
		// We use setTimeout to sequence this drop after any potential users in the current call stack progression, since this will begin in an entirely new call stack later
		setTimeout(() => {
			document.removeEventListener("drop", clearDraggingElement);
		}, 0);
	};
}

export function beginDraggingElement(element: HTMLElement) {
	draggingElement = element;
}

export function currentDraggingElement(): HTMLElement | undefined {
	return draggingElement;
}
