// Allow JS import statements to work with .vue files
declare module "*.vue" {
	const component: DefineComponent;
	export default component;
}

// Allow JS import statements to work with .svg files
declare module "*.svg" {
	const component: DefineComponent;
	export default component;
}
