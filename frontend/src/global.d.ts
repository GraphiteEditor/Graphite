/* eslint-disable @typescript-eslint/consistent-type-definitions */

// Graphite's custom properties added to the global `window` object
interface Window {
	imageCanvases: Record<string, HTMLCanvasElement>;
	receiveNativeMessage?: (buffer: ArrayBuffer) => void;
}

// Experimental Keyboard API: https://developer.mozilla.org/en-US/docs/Web/API/Keyboard
interface Navigator {
	keyboard?: Keyboard;
}
interface Keyboard {
	lock(keyCodes?: string[]): Promise<void>;
	unlock(): void;
	getLayoutMap(): Promise<KeyboardLayoutMap>;
}
interface KeyboardLayoutMap {
	entries(): IterableIterator<[string, string]>;
	get(key: string): string | undefined;
	has(key: string): boolean;
	readonly size: number;
}

// Experimental EyeDropper API: https://developer.mozilla.org/en-US/docs/Web/API/EyeDropper
interface Window {
	EyeDropper?: typeof EyeDropper;
}
declare class EyeDropper {
	constructor();
	open(options?: { signal?: AbortSignal }): Promise<{ sRGBHex: string }>;
}

// Non-standard Stack Trace Limit API: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error/stackTraceLimit
interface ErrorConstructor {
	stackTraceLimit?: number;
}
