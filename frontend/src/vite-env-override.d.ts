// Allow `import` statements to work with SVG files in the eyes of the TypeScript compiler.
// This prevents red underlines from showing and lets it know the types of imported variables are strings.
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
