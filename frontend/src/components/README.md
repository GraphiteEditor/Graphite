# Overview of `/frontend/src/components/`
Each component represents a (usually reusable) part of the Graphite Editor GUI. These all get mounted within the Vue entry point, `App.vue`, in the `/src` directory above this one.

## Floating Menus: `floating-menus/`
The temporary UI areas with dark backgrounds which hover over the top of the editor window content. Examples include popovers, dropdown menu selectors, and dialog modals.

## Layout: `layout/`
Useful containers that control the flow of content held within.

## Panels: `panels/`
The dockable tabbed regions like the Document, Properties, Layer Tree, and Node Graph panels.

## Widgets: `widgets/`
The interactive input items used to display information and provide user control.

## Window: `window/`
The building blocks for the Title Bar, Workspace, and Status Bar within an editor application window.

# Vue tips and tricks
This section contains a growing list of quick reference information for helpful Vue solutions and best practices. Feel free to add to this to help contributors learn things, or yourself remember tricks you'll likely forget in a few months.

## Bi-directional props
The component declares this:
```ts
export default defineComponent({
	emits: ["update:theBidirectionalProperty"],
	props: {
		theBidirectionalProperty: { type: Number as PropType<number>, required: false },
	},
	watch: {
		// Called only when `theBidirectionalProperty` is changed from outside this component (with v-model)
		theBidirectionalProperty(newSelectedIndex: number | undefined) {

		},
	},
	methods: {
		doSomething() {
			this.$emit("update:theBidirectionalProperty", SOME_NEW_VALUE);
		},
	},
});
```

Users of the component do this for `theCorrespondingDataEntry` to be a two-way binding:
```html
<DropdownInput v-model:theBidirectionalProperty="theCorrespondingDataEntry" />
```
