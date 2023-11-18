# Overview of `/frontend/src/components/`

Each component represents a (usually reusable) part of the Graphite editor GUI. These all get mounted in `Editor.svelte` (in the `/src` directory above this one).

## Floating Menus: `floating-menus/`

The temporary UI areas with dark backgrounds which hover over the top of the editor window content. Examples include menu lists, popovers, and dialogs.

## Layout: `layout/`

Useful containers that control the flow of content held within.

## Panels: `panels/`

The dockable tabbed regions like the Document, Properties, Layers, and Node Graph panels.

## Widgets: `widgets/`

The interactive input items used to display information and provide user control.

## Window: `window/`

The building blocks for the Title Bar, Workspace, and Status Bar within an editor application window.

# Svelte tips and tricks

This section contains a growing list of quick reference information for helpful Svelte solutions and best practices. Feel free to add to this to help contributors learn things, or yourself remember tricks you'll likely forget in a few months.

## Bi-directional props

The component declares this:

```ts
// The dispatcher that sends the changed value as a custom event to the parent
const dispatch = createEventDispatcher<{ theBidirectionalProperty: number }>();

// The prop
export let theBidirectionalProperty: number;

// Called only when `theBidirectionalProperty` is changed from outside this component via its props
$: console.log(theBidirectionalProperty);

// Example of a method that would update the value
function doSomething() {
	dispatch("theBidirectionalProperty", SOME_NEW_VALUE);
},
```

Users of the component do this for `theCorrespondingDataEntry` to be a two-way binding:

```ts
let theCorrespondingDataEntry = 42;
```

```svelte
<DropdownInput
	theBidirectionalProperty={theCorrespondingDataEntry}
	on:theBidirectionalProperty={({ detail }) => { theCorrespondingDataEntry = detail; }}
/>
```
