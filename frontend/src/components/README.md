# Vue components

Each component is a layout or widget in the GUI.

This document is a growing list of quick reference information for helpful Vue solutions and best practices. Feel free to add to this to help contributors learn things, or yourself remember tricks you'll likely forget in a few months.

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
