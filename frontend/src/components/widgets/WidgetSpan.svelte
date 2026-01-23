<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { LayoutTarget, WidgetInstance, WidgetSpanColumn, WidgetSpanRow } from "@graphite/messages";
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
	import NumberInput from "@graphite/components/widgets/inputs/NumberInput.svelte";
	import RadioInput from "@graphite/components/widgets/inputs/RadioInput.svelte";
	import ReferencePointInput from "@graphite/components/widgets/inputs/ReferencePointInput.svelte";
	import TextAreaInput from "@graphite/components/widgets/inputs/TextAreaInput.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import WorkingColorsInput from "@graphite/components/widgets/inputs/WorkingColorsInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import ImageLabel from "@graphite/components/widgets/labels/ImageLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import ShortcutLabel from "@graphite/components/widgets/labels/ShortcutLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const editor = getContext<Editor>("editor");

	export let widgetData: WidgetSpanRow | WidgetSpanColumn;
	export let layoutTarget: LayoutTarget;

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

	function watchWidgets(widgetData: WidgetSpanRow | WidgetSpanColumn): WidgetInstance[] {
		let widgets: WidgetInstance[] = [];
		if (isWidgetSpanRow(widgetData)) widgets = widgetData.rowWidgets;
		else if (isWidgetSpanColumn(widgetData)) widgets = widgetData.columnWidgets;
		return widgets;
	}

	function widgetValueCommit(widgetIndex: number, value: unknown) {
		editor.handle.widgetValueCommit(layoutTarget, widgets[widgetIndex].widgetId, value);
	}

	function widgetValueUpdate(widgetIndex: number, value: unknown, resendWidget: boolean) {
		editor.handle.widgetValueUpdate(layoutTarget, widgets[widgetIndex].widgetId, value, resendWidget);
	}

	function widgetValueCommitAndUpdate(widgetIndex: number, value: unknown, resendWidget: boolean) {
		editor.handle.widgetValueCommitAndUpdate(layoutTarget, widgets[widgetIndex].widgetId, value, resendWidget);
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
	{#each widgets as component, widgetIndex}
		{@const checkboxInput = narrowWidgetProps(component.props, "CheckboxInput")}
		{#if checkboxInput}
			<CheckboxInput {...exclude(checkboxInput)} on:checked={({ detail }) => widgetValueCommitAndUpdate(widgetIndex, detail, true)} />
		{/if}
		{@const colorInput = narrowWidgetProps(component.props, "ColorInput")}
		{#if colorInput}
			<ColorInput
				{...exclude(colorInput)}
				on:value={({ detail }) => widgetValueUpdate(widgetIndex, detail, false)}
				on:startHistoryTransaction={() => widgetValueCommit(widgetIndex, colorInput.value)}
			/>
		{/if}
		<!-- TODO: Curves Input is currently unused -->
		{@const curvesInput = narrowWidgetProps(component.props, "CurveInput")}
		{#if curvesInput}
			<CurveInput
				{...exclude(curvesInput)}
				on:value={({ detail }) => debouncer((value) => widgetValueCommitAndUpdate(widgetIndex, value, false), { debounceTime: 120 }).debounceUpdateValue(detail)}
			/>
		{/if}
		{@const dropdownInput = narrowWidgetProps(component.props, "DropdownInput")}
		{#if dropdownInput}
			<DropdownInput
				{...exclude(dropdownInput)}
				on:hoverInEntry={({ detail }) => {
					return widgetValueUpdate(widgetIndex, detail, false);
				}}
				on:hoverOutEntry={({ detail }) => {
					return widgetValueUpdate(widgetIndex, detail, false);
				}}
				on:selectedIndex={({ detail }) => widgetValueCommitAndUpdate(widgetIndex, detail, true)}
			/>
		{/if}
		{@const parameterExposeButton = narrowWidgetProps(component.props, "ParameterExposeButton")}
		{#if parameterExposeButton}
			<ParameterExposeButton {...exclude(parameterExposeButton)} action={() => widgetValueCommitAndUpdate(widgetIndex, undefined, true)} />
		{/if}
		{@const iconButton = narrowWidgetProps(component.props, "IconButton")}
		{#if iconButton}
			<IconButton {...exclude(iconButton)} action={() => widgetValueCommitAndUpdate(widgetIndex, undefined, true)} />
		{/if}
		{@const iconLabel = narrowWidgetProps(component.props, "IconLabel")}
		{#if iconLabel}
			<IconLabel {...exclude(iconLabel)} />
		{/if}
		{@const shortcutLabel = narrowWidgetProps(component.props, "ShortcutLabel")}
		{@const shortcutLabelShortcut = shortcutLabel?.shortcut ? { ...shortcutLabel, shortcut: shortcutLabel.shortcut } : undefined}
		{#if shortcutLabel && shortcutLabelShortcut}
			<ShortcutLabel {...exclude(shortcutLabelShortcut)} />
		{/if}
		{@const imageLabel = narrowWidgetProps(component.props, "ImageLabel")}
		{#if imageLabel}
			<ImageLabel {...exclude(imageLabel)} />
		{/if}
		{@const imageButton = narrowWidgetProps(component.props, "ImageButton")}
		{#if imageButton}
			<ImageButton {...exclude(imageButton)} action={() => widgetValueCommitAndUpdate(widgetIndex, undefined, true)} />
		{/if}
		{@const nodeCatalog = narrowWidgetProps(component.props, "NodeCatalog")}
		{#if nodeCatalog}
			<NodeCatalog {...exclude(nodeCatalog)} on:selectNodeType={(e) => widgetValueCommitAndUpdate(widgetIndex, e.detail, false)} />
		{/if}
		{@const numberInput = narrowWidgetProps(component.props, "NumberInput")}
		{#if numberInput}
			<NumberInput
				{...exclude(numberInput)}
				on:value={({ detail }) => debouncer((value) => widgetValueUpdate(widgetIndex, value, true)).debounceUpdateValue(detail)}
				on:startHistoryTransaction={() => widgetValueCommit(widgetIndex, numberInput.value)}
				incrementCallbackIncrease={() => widgetValueCommitAndUpdate(widgetIndex, "Increment", false)}
				incrementCallbackDecrease={() => widgetValueCommitAndUpdate(widgetIndex, "Decrement", false)}
			/>
		{/if}
		{@const referencePointInput = narrowWidgetProps(component.props, "ReferencePointInput")}
		{#if referencePointInput}
			<ReferencePointInput {...exclude(referencePointInput)} on:value={({ detail }) => widgetValueCommitAndUpdate(widgetIndex, detail, true)} />
		{/if}
		{@const popoverButton = narrowWidgetProps(component.props, "PopoverButton")}
		{#if popoverButton}
			<PopoverButton {...exclude(popoverButton)} {layoutTarget} />
		{/if}
		{@const radioInput = narrowWidgetProps(component.props, "RadioInput")}
		{#if radioInput}
			<RadioInput {...exclude(radioInput)} on:selectedIndex={({ detail }) => widgetValueCommitAndUpdate(widgetIndex, detail, true)} />
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
			<TextAreaInput {...exclude(textAreaInput)} on:commitText={({ detail }) => widgetValueCommitAndUpdate(widgetIndex, detail, false)} />
		{/if}
		{@const textButton = narrowWidgetProps(component.props, "TextButton")}
		{#if textButton}
			<TextButton
				{...exclude(textButton)}
				action={() => widgetValueCommitAndUpdate(widgetIndex, [], true)}
				on:selectedEntryValuePath={({ detail }) => widgetValueCommitAndUpdate(widgetIndex, detail, false)}
			/>
		{/if}
		{@const breadcrumbTrailButtons = narrowWidgetProps(component.props, "BreadcrumbTrailButtons")}
		{#if breadcrumbTrailButtons}
			<BreadcrumbTrailButtons {...exclude(breadcrumbTrailButtons)} action={(breadcrumbIndex) => widgetValueCommitAndUpdate(widgetIndex, breadcrumbIndex, true)} />
		{/if}
		{@const textInput = narrowWidgetProps(component.props, "TextInput")}
		{#if textInput}
			<TextInput {...exclude(textInput)} on:commitText={({ detail }) => widgetValueCommitAndUpdate(widgetIndex, detail, true)} />
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
