# GUI system

This directory contains the XML files describing the components which make up Graphite's GUI.

## Principles

The framework is inspired by [Vue.js](https://vuejs.org/).
Each component's layout is defined in an XML, and recursively made out of lower-level components.

Interactivity is provided by script files which expose reactive variables. As these variables are mutated, the component is updated to match the current state.

## Layout

The layout engine does a top-down pass through the component tree in order to determine what to render.

Layout is controlled using predefined attributes, such as `width`, `height`, `x-align`, `y-align`, `spacing` or `padding`.

## Component lifetime

The children of a component are passed to it as a `content` attribute. For example, looking at the row component:
```xml
<row content="INNER_XML: (GuiXml | None) = none">
    {{INNER_XML}}
</row>
```
The `content` attribute defines a new variable `INNER_XML` of type either `GuiXml` or `None`, which can contain more XML or nothing at all. It has a default value of `none` (of type `None`).
This is then expanded in the body of the row: `{{INNER_XML}}`.

## Defining new components

### Component files

To define a new component, create a new `.xml` file in this directory. Subdirectories become namespaces for the components (e.g. the file `window/main.xml` defines a component `<window:main>`).

### Parameters

User-defined parameters start with a colon (`:`).

They are created by adding attributes to a component source file:
`:parameter="VARIABLE_NAME: (VariableType) = defaultValue"`
