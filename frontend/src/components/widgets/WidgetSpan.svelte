<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { Widget, WidgetSpanColumn, WidgetSpanRow } from "@graphite/messages";
	import { narrowWidgetProps, isWidgetSpanColumn, isWidgetSpanRow } from "@graphite/messages";
	import { debouncer } from "@graphite/utility-functions/debounce";

	import NodeCatalog from "@graphite/components/floating-menus/NodeCatalog.svelte";
	import BreadcrumbTrailButtons from "@graphite/components/widgets/buttons/BreadcrumbTrailButtons.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import ImageButton from "@graphite/components/widgets/buttons/ImageButton.svelte";
	import ParameterExposeButton from "@graphite/components/widgets/buttons/ParameterExposeButton.svelte";
	import PopoverButton from "@graphite/components/widgets/buttons/PopoverButton.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import CheckboxInput from "@graphite/components/widgets/inputs/CheckboxInput.svelte";
	import ColorInput from "@graphite/components/widgets/inputs/ColorInput.svelte";
	import CurveInput from "@graphite/components/widgets/inputs/CurveInput.svelte";
	import DropdownInput from "@graphite/components/widgets/inputs/DropdownInput.svelte";
	import FontInput from "@graphite/components/widgets/inputs/FontInput.svelte";
	import NumberInput from "@graphite/components/widgets/inputs/NumberInput.svelte";
	import RadioInput from "@graphite/components/widgets/inputs/RadioInput.svelte";
	import ReferencePointInput from "@graphite/components/widgets/inputs/ReferencePointInput.svelte";
	import TextAreaInput from "@graphite/components/widgets/inputs/TextAreaInput.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import WorkingColorsInput from "@graphite/components/widgets/inputs/WorkingColorsInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import ImageLabel from "@graphite/components/widgets/labels/ImageLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	const editor = getContext<Editor>("editor");

	export let widgetData: WidgetSpanRow | WidgetSpanColumn;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	export let layoutTarget: any;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let narrow = false;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");

	$: direction = watchDirection(widgetData);
	$: widgets = watchWidgets(widgetData);

	function watchDirection(widgetData: WidgetSpanRow | WidgetSpanColumn): "row" | "column" | undefined {
		if (isWidgetSpanRow(widgetData)) return "row";
		if (isWidgetSpanColumn(widgetData)) return "column";
	}

	function watchWidgets(widgetData: WidgetSpanRow | WidgetSpanColumn): Widget[] {
		let widgets: Widget[] = [];
		if (isWidgetSpanRow(widgetData)) widgets = widgetData.rowWidgets;
		else if (isWidgetSpanColumn(widgetData)) widgets = widgetData.columnWidgets;
		return widgets;
	}

	function widgetValueCommit(index: number, value: unknown) {
		editor.handle.widgetValueCommit(layoutTarget, widgets[index].widgetId, value);
	}

	function widgetValueUpdate(index: number, value: unknown) {
		editor.handle.widgetValueUpdate(layoutTarget, widgets[index].widgetId, value);
	}

	function widgetValueCommitAndUpdate(index: number, value: unknown) {
		editor.handle.widgetValueCommitAndUpdate(layoutTarget, widgets[index].widgetId, value);
	}

	// TODO: This seems to work, but verify the correctness and terseness of this, it's adapted from https://stackoverflow.com/a/67434028/775283
	function exclude<T extends object>(props: T, additional?: (keyof T)[]): Omit<T, typeof additional extends Array<infer K> ? K : never> {
		const exclusions = ["kind", ...(additional || [])];

		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		return Object.fromEntries(Object.entries(props).filter((entry) => !exclusions.includes(entry[0]))) as any;
	}
</script>

<!-- TODO: Refactor this component to use `<svelte:component this={attributesObject} />` to avoid all the separate conditional components -->

<div class={`widget-span ${className} ${extraClasses}`.trim()} class:narrow class:row={direction === "row"} class:column={direction === "column"}>
	{#each widgets as component, index}
		{@const checkboxInput = narrowWidgetProps(component.props, "CheckboxInput")}
		{#if checkboxInput}
			<CheckboxInput {...exclude(checkboxInput)} on:checked={({ detail }) => widgetValueCommitAndUpdate(index, detail)} />
		{/if}
		{@const colorInput = narrowWidgetProps(component.props, "ColorInput")}
		{#if colorInput}
			<ColorInput {...exclude(colorInput)} on:value={({ detail }) => widgetValueUpdate(index, detail)} on:startHistoryTransaction={() => widgetValueCommit(index, colorInput.value)} />
		{/if}
		{@const curvesInput = narrowWidgetProps(component.props, "CurveInput")}
		{#if curvesInput}
			<CurveInput {...exclude(curvesInput)} on:value={({ detail }) => debouncer((value) => widgetValueCommitAndUpdate(index, value), { debounceTime: 120 }).debounceUpdateValue(detail)} />
		{/if}
		{@const dropdownInput = narrowWidgetProps(component.props, "DropdownInput")}
		{#if dropdownInput}
			<DropdownInput
				{...exclude(dropdownInput)}
				on:hoverInEntry={({ detail }) => {
					return widgetValueUpdate(index, detail);
				}}
				on:hoverOutEntry={({ detail }) => {
					return widgetValueUpdate(index, detail);
				}}
				on:selectedIndex={({ detail }) => widgetValueCommitAndUpdate(index, detail)}
			/>
		{/if}
		{@const fontInput = narrowWidgetProps(component.props, "FontInput")}
		{#if fontInput}
			<FontInput {...exclude(fontInput)} on:changeFont={({ detail }) => widgetValueCommitAndUpdate(index, detail)} />
		{/if}
		{@const parameterExposeButton = narrowWidgetProps(component.props, "ParameterExposeButton")}
		{#if parameterExposeButton}
			<ParameterExposeButton {...exclude(parameterExposeButton)} action={() => widgetValueCommitAndUpdate(index, undefined)} />
		{/if}
		{@const iconButton = narrowWidgetProps(component.props, "IconButton")}
		{#if iconButton}
			<IconButton {...exclude(iconButton)} action={() => widgetValueCommitAndUpdate(index, undefined)} />
		{/if}
		{@const iconLabel = narrowWidgetProps(component.props, "IconLabel")}
		{#if iconLabel}
			<IconLabel {...exclude(iconLabel)} />
		{/if}
		{@const imageLabel = narrowWidgetProps(component.props, "ImageLabel")}
		{#if imageLabel}
			<ImageLabel {...exclude(imageLabel)} />
		{/if}
		{@const imageButton = narrowWidgetProps(component.props, "ImageButton")}
		{#if imageButton}
			<ImageButton {...exclude(imageButton)} action={() => widgetValueCommitAndUpdate(index, undefined)} />
		{/if}
		{@const nodeCatalog = narrowWidgetProps(component.props, "NodeCatalog")}
		{#if nodeCatalog}
			<NodeCatalog {...exclude(nodeCatalog)} on:selectNodeType={(e) => widgetValueCommitAndUpdate(index, e.detail)} />
		{/if}
		{@const numberInput = narrowWidgetProps(component.props, "NumberInput")}
		{#if numberInput}
			<NumberInput
				{...exclude(numberInput)}
				on:value={({ detail }) => debouncer((value) => widgetValueUpdate(index, value)).debounceUpdateValue(detail)}
				on:startHistoryTransaction={() => widgetValueCommit(index, numberInput.value)}
				incrementCallbackIncrease={() => widgetValueCommitAndUpdate(index, "Increment")}
				incrementCallbackDecrease={() => widgetValueCommitAndUpdate(index, "Decrement")}
			/>
		{/if}
		{@const referencePointInput = narrowWidgetProps(component.props, "ReferencePointInput")}
		{#if referencePointInput}
			<ReferencePointInput {...exclude(referencePointInput)} on:value={({ detail }) => widgetValueCommitAndUpdate(index, detail)} />
		{/if}
		{@const popoverButton = narrowWidgetProps(component.props, "PopoverButton")}
		{#if popoverButton}
			<PopoverButton {...exclude(popoverButton, ["popoverLayout"])}>
				<WidgetLayout layout={{ layout: popoverButton.popoverLayout, layoutTarget: layoutTarget }} />
			</PopoverButton>
		{/if}
		{@const radioInput = narrowWidgetProps(component.props, "RadioInput")}
		{#if radioInput}
			<RadioInput {...exclude(radioInput)} on:selectedIndex={({ detail }) => widgetValueCommitAndUpdate(index, detail)} />
		{/if}
		{@const separator = narrowWidgetProps(component.props, "Separator")}
		{#if separator}
			<Separator {...exclude(separator)} />
		{/if}
		{@const workingColorsInput = narrowWidgetProps(component.props, "WorkingColorsInput")}
		{#if workingColorsInput}
			<WorkingColorsInput {...exclude(workingColorsInput)} />
		{/if}
		{@const textAreaInput = narrowWidgetProps(component.props, "TextAreaInput")}
		{#if textAreaInput}
			<TextAreaInput {...exclude(textAreaInput)} on:commitText={({ detail }) => widgetValueCommitAndUpdate(index, detail)} />
		{/if}
		{@const textButton = narrowWidgetProps(component.props, "TextButton")}
		{#if textButton}
			<TextButton {...exclude(textButton)} action={() => widgetValueCommitAndUpdate(index, undefined)} />
		{/if}
		{@const breadcrumbTrailButtons = narrowWidgetProps(component.props, "BreadcrumbTrailButtons")}
		{#if breadcrumbTrailButtons}
			<BreadcrumbTrailButtons {...exclude(breadcrumbTrailButtons)} action={(breadcrumbIndex) => widgetValueCommitAndUpdate(index, breadcrumbIndex)} />
		{/if}
		{@const textInput = narrowWidgetProps(component.props, "TextInput")}
		{#if textInput}
			<TextInput {...exclude(textInput)} on:commitText={({ detail }) => widgetValueCommitAndUpdate(index, detail)} />
		{/if}
		{@const textLabel = narrowWidgetProps(component.props, "TextLabel")}
		{#if textLabel}
			<TextLabel {...exclude(textLabel, ["value"])}>{textLabel.value}</TextLabel>
		{/if}
	{/each}
</div>

<style lang="scss" global>
	.widget-span.column {
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
	}

	.widget-span.row {
		flex: 0 0 auto;
		display: flex;
		--row-height: 32px;
		min-height: var(--row-height);

		&.narrow {
			--row-height: 24px;
		}

		> * {
			--widget-height: 24px;
			// Vertically center the widget within the row
			margin: calc((var(--row-height) - var(--widget-height)) / 2) 0;
			min-height: var(--widget-height);

			&:not(.multiline) {
				line-height: var(--widget-height);
			}

			&.icon-label.size-12 {
				--widget-height: 12px;
			}

			&.icon-label.size-16 {
				--widget-height: 16px;
			}
		}
	}
</style>
