# Bi-directional props in Vue

The component declares this:
```ts
export default defineComponent({
	emits: ["update:selectedIndex"],
	props: {
		selectedIndex: { type: Number as PropType<number>, required: false },
	},
	watch: {
		// Called only when `selectedIndex` is changed from outside this component (with v-model)
		selectedIndex(newSelectedIndex: number | undefined) {

		},
	},
	methods: {
		doSomething() {
			this.$emit("update:selectedIndex", SOME_NEW_VALUE);
		},
	},
});
```

Users of the component do this for `documentModeSelectionIndex` to be a two-way binding:
```vue
<DropdownInput v-model:selectedIndex="documentModeSelectionIndex" />
```
