# GUI system

This directory contains the XML files describing the components which make up Graphite's GUI.

## Principles

The framework is inspired by [Vue.js](https://vuejs.org/).
Each component's layout is defined in an XML, and recursively made out of lower-level components.

Interactivity is provided by script files which expose reactive variables. As these variables are mutated, the component is updated to match the current state.

## Layout

The layout engine does a top-down pass through the component tree in order to determine what to render.

Layout is controlled using predefined attributes, such as `width`, `height`, `x-align`, `y-align`, `gap` or `padding`.

### Layout algorithm

To calculate dimensions (width/height):
- elements with **fixed** sizes (e.g. `123px`) are laid out, in the order they are encountered.
- then we handle elements with `inner`, which indicates that the size of the component depends on the size of the child. Their sizes have to be recursively computed. Afterwards we add their padding and spacing.
- then we handle elements with **percentage** sizes (such as `10%`), based on the computed total size of the parent container.
- then `@` rules (e.g. `100@`) are applied, which divide up the remaining free space in the container.

When the `width`/`height` attributes are not specified, they each default to `inner`.

If there's not enough space in the parent container to lay out all children, the container overflows (this can be handled, for example, through _scrolling_).

To calculate positions (x/y):
- this only makes sense when there is some free space left (otherwise, all the elements fit tightly together and are positioned one after another).
- `x-align`/`y-align` take a percentage, which indicates where it should be along the respective axis.
  `0%` would mean completely to the left/to the top, `100%` would mean completely to the right/to the bottom, and `50%` would be halfway between.

## Component lifetime

The children of a component are passed to it as a `content` attribute. For example, looking at the row component:
```xml
<row content="INNER_XML: (Layout) = [[]]">
    {{INNER_XML}}
</row>
```
The `content` attribute defines a new variable `INNER_XML` of type `Layout` which can contain more XML layout structure. It has a default value of `[[]]` which refers to an empty layout— XML syntax (for the `Layout` data type) written in a tag's attribute is wrapped in ``[[`` (opening) and `]]` (closing) symbols. In this case the `INNER_XML` variable defaults to empty XML, however it is not stricly useful here because the `content` attribute will always have its value replaced by whatever exists between opening and closing tags when this component is called from elsewhere.

This is then expanded in the body of the row: `{{INNER_XML}}`.

## Defining new components

### Component files

To define a new component, create a new `.xml` file in this directory. Subdirectories become namespaces for the components (e.g. the file `window/main.xml` defines a component `<window:main>`).

### Parameters

User-defined parameters start with a colon (`:`).

They are created by adding attributes to a component source file:
`:parameter="VARIABLE_NAME: (VariableType) = defaultValue"`
