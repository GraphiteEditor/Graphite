// Allow `import` statements to work with image files in the eyes of the TypeScript compiler.
// This prevents red underlines from showing and lets it know the types of imported variables for image data.
// The actual import is performed by Vite when building, as configured in the `resolve` aliases in `vite.config.ts`.

declare module "*.svg" {
	const content: string;
	export default content;
}

declare module "*.png" {
	const content: string;
	export default content;
}

declare module "*.jpg" {
	const content: string;
	export default content;
}

// Stub declarations for modules generated at build time.
// These allow TypeScript to compile without the actual modules present.
// The real types are provided by wasm-pack when the WASM is built.
declare module "@graphite/../wasm/pkg/graphite_wasm" {
	/* eslint-disable @typescript-eslint/no-explicit-any */
	export class EditorHandle {
		static create(os: string, randomSeed: bigint, callback: (messageType: string, messageData: Record<string, unknown>) => void): EditorHandle;
		[key: string]: any;
	}
	export function wasmMemory(): WebAssembly.Memory;
	export function receiveNativeMessage(message: string): void;
	export function isPlatformNative(): boolean;
	export function evaluateMathExpression(expression: string): number | undefined;
	export default function init(): Promise<Record<string, () => void>>;
	/* eslint-enable @typescript-eslint/no-explicit-any */
}

declare module "@branding/*" {
	const content: string;
	export default content;
}
