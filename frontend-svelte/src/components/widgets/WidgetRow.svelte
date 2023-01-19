<script lang="ts">
	import { debouncer } from "@/utility-functions/debounce";
	import type { Widget } from "@/wasm-communication/messages";
	import { narrowWidgetProps } from "@/wasm-communication/messages";
	import { isWidgetColumn, isWidgetRow, type WidgetColumn, type WidgetRow } from "@/wasm-communication/messages";

	import PivotAssist from "@/components/widgets/assists/PivotAssist.svelte";
	import BreadcrumbTrailButtons from "@/components/widgets/buttons/BreadcrumbTrailButtons.svelte";
	import IconButton from "@/components/widgets/buttons/IconButton.svelte";
	import ParameterExposeButton from "@/components/widgets/buttons/ParameterExposeButton.svelte";
	import PopoverButton from "@/components/widgets/buttons/PopoverButton.svelte";
	import TextButton from "@/components/widgets/buttons/TextButton.svelte";
	import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.svelte";
	import ColorInput from "@/components/widgets/inputs/ColorInput.svelte";
	import DropdownInput from "@/components/widgets/inputs/DropdownInput.svelte";
	import FontInput from "@/components/widgets/inputs/FontInput.svelte";
	import LayerReferenceInput from "@/components/widgets/inputs/LayerReferenceInput.svelte";
	import NumberInput from "@/components/widgets/inputs/NumberInput.svelte";
	import OptionalInput from "@/components/widgets/inputs/OptionalInput.svelte";
	import RadioInput from "@/components/widgets/inputs/RadioInput.svelte";
	import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.svelte";
	import TextAreaInput from "@/components/widgets/inputs/TextAreaInput.svelte";
	import TextInput from "@/components/widgets/inputs/TextInput.svelte";
	import IconLabel from "@/components/widgets/labels/IconLabel.svelte";
	import Separator from "@/components/widgets/labels/Separator.svelte";
	import TextLabel from "@/components/widgets/labels/TextLabel.svelte";
	import { getContext } from "svelte";
	import type { Editor } from "@/wasm-communication/editor";

	const SUFFIX_WIDGETS = ["PopoverButton"];

	const editor = getContext<Editor>("editor");

	export let widgetData: WidgetColumn | WidgetRow;
	export let layoutTarget: any;

	$: direction = watchDirection(widgetData);
	$: widgets = watchWidgets(widgetData);
	$: widgetsAndNextSiblingIsSuffix = watchWidgetsAndNextSiblingIsSuffix(widgets);

	function watchDirection(widgetData: WidgetRow | WidgetColumn): "row" | "column" | "ERROR" {
		if (isWidgetRow(widgetData)) return "row";
		if (isWidgetColumn(widgetData)) return "column";
		return "ERROR";
	}

	function watchWidgets(widgetData: WidgetRow | WidgetColumn): Widget[] {
		let widgets: Widget[] = [];
		if (isWidgetRow(widgetData)) widgets = widgetData.rowWidgets;
		else if (isWidgetColumn(widgetData)) widgets = widgetData.columnWidgets;
		return widgets;
	}

	function watchWidgetsAndNextSiblingIsSuffix(widgets: Widget[]): [Widget, boolean][] {
		return widgets.map((widget, index): [Widget, boolean] => {
			// A suffix widget is one that joins up with this widget at the end with only a 1px gap.
			// It uses the CSS sibling selector to give its own left edge corners zero radius.
			// But this JS is needed to set its preceding sibling widget's right edge corners to zero radius.
			const nextSiblingIsSuffix = SUFFIX_WIDGETS.includes(widgets[index + 1]?.props.kind);

			return [widget, nextSiblingIsSuffix];
		});
	}

	function updateLayout(index: number, value: unknown) {
		editor.instance.updateLayout(layoutTarget, widgets[index].widgetId, value);
	}

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	// function exclude<T extends Record<string, any>>(props: T, additional?: (keyof T)[]): Pick<T, Exclude<keyof T, "kind" | (typeof additional extends Array<infer K> ? K : never)>> {
	// 	const exclusions = ["kind", ...(additional || [])];

	// 	return Object.fromEntries(Object.entries(props).filter((entry) => !exclusions.includes(entry[0]))) as any;
	// }

	// TODO: This seems to work, but verify the correctness and terseness of this, it's adapted from https://stackoverflow.com/a/67434028/775283
	function exclude<T extends object>(props: T, additional?: (keyof T)[]): Omit<T, typeof additional extends Array<infer K> ? K : never> {
		const exclusions = ["kind", ...(additional || [])];

		return Object.fromEntries(Object.entries(props).filter((entry) => !exclusions.includes(entry[0]))) as any;
	}
</script>

<!-- TODO: Refactor this component to use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
<!-- TODO: Also rename this component, and probably move the `widget-${direction}` wrapper to be part of `WidgetLayout.svelte` as part of its refactor -->

<div class={`widget-${direction}`}>
	{#each widgetsAndNextSiblingIsSuffix as [component, nextIsSuffix], index (index)}
		{@const checkboxInput = narrowWidgetProps(component.props, "CheckboxInput")}
		{#if checkboxInput}
			<CheckboxInput {...exclude(checkboxInput)} on:checked={({ detail }) => updateLayout(index, detail)} />
		{/if}
		{@const colorInput = narrowWidgetProps(component.props, "ColorInput")}
		{#if colorInput}
			<ColorInput {...exclude(colorInput)} on:value={({ detail }) => updateLayout(index, detail)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{@const dropdownInput = narrowWidgetProps(component.props, "DropdownInput")}
		{#if dropdownInput}
			<DropdownInput {...exclude(dropdownInput)} on:selectedIndex={({ detail }) => updateLayout(index, detail)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{@const fontInput = narrowWidgetProps(component.props, "FontInput")}
		{#if fontInput}
			<FontInput {...exclude(fontInput)} on:changeFont={({ detail }) => updateLayout(index, detail)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{@const parameterExposeButton = narrowWidgetProps(component.props, "ParameterExposeButton")}
		{#if parameterExposeButton}
			<ParameterExposeButton {...exclude(parameterExposeButton)} action={() => updateLayout(index, undefined)} />
		{/if}
		{@const iconButton = narrowWidgetProps(component.props, "IconButton")}
		{#if iconButton}
			<IconButton {...exclude(iconButton)} action={() => updateLayout(index, undefined)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{@const iconLabel = narrowWidgetProps(component.props, "IconLabel")}
		{#if iconLabel}
			<IconLabel {...exclude(iconLabel)} />
		{/if}
		{@const layerReferenceInput = narrowWidgetProps(component.props, "LayerReferenceInput")}
		{#if layerReferenceInput}
			<LayerReferenceInput {...exclude(layerReferenceInput)} on:value={({ detail }) => updateLayout(index, detail)} />
		{/if}
		{@const numberInput = narrowWidgetProps(component.props, "NumberInput")}
		{#if numberInput}
			<NumberInput
				{...exclude(numberInput)}
				on:value={({ detail }) => debouncer(() => updateLayout(index, detail))}
				incrementCallbackIncrease={() => updateLayout(index, "Increment")}
				incrementCallbackDecrease={() => updateLayout(index, "Decrement")}
				sharpRightCorners={nextIsSuffix}
			/>
		{/if}
		{@const optionalInput = narrowWidgetProps(component.props, "OptionalInput")}
		{#if optionalInput}
			<OptionalInput {...exclude(optionalInput)} on:checked={({ detail }) => updateLayout(index, detail)} />
		{/if}
		{@const pivotAssist = narrowWidgetProps(component.props, "PivotAssist")}
		{#if pivotAssist}
			<PivotAssist {...exclude(pivotAssist)} on:position={({ detail }) => updateLayout(index, detail)} />
		{/if}
		{@const popoverButton = narrowWidgetProps(component.props, "PopoverButton")}
		{#if popoverButton}
			<PopoverButton {...exclude(popoverButton, ["header", "text"])}>
				<TextLabel bold={true}>{popoverButton.header}</TextLabel>
				<TextLabel multiline={true}>{popoverButton.text}</TextLabel>
			</PopoverButton>
		{/if}
		{@const radioInput = narrowWidgetProps(component.props, "RadioInput")}
		{#if radioInput}
			<RadioInput {...exclude(radioInput)} on:selectedIndex={({ detail }) => updateLayout(index, detail)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{@const separator = narrowWidgetProps(component.props, "Separator")}
		{#if separator}
			<Separator {...exclude(separator)} />
		{/if}
		{@const swatchPairInput = narrowWidgetProps(component.props, "SwatchPairInput")}
		{#if swatchPairInput}
			<SwatchPairInput {...exclude(swatchPairInput)} />
		{/if}
		{@const textAreaInput = narrowWidgetProps(component.props, "TextAreaInput")}
		{#if textAreaInput}
			<TextAreaInput {...exclude(textAreaInput)} on:commitText={({ detail }) => updateLayout(index, detail)} />
		{/if}
		{@const textButton = narrowWidgetProps(component.props, "TextButton")}
		{#if textButton}
			<TextButton {...exclude(textButton)} action={() => updateLayout(index, undefined)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{@const breadcrumbTrailButtons = narrowWidgetProps(component.props, "BreadcrumbTrailButtons")}
		{#if breadcrumbTrailButtons}
			<BreadcrumbTrailButtons {...exclude(breadcrumbTrailButtons)} action={(index) => updateLayout(index, index)} />
		{/if}
		{@const textInput = narrowWidgetProps(component.props, "TextInput")}
		{#if textInput}
			<TextInput {...exclude(textInput)} on:commitText={({ detail }) => updateLayout(index, detail)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{@const textLabel = narrowWidgetProps(component.props, "TextLabel")}
		{#if textLabel}
			<TextLabel {...exclude(textLabel, ["value"])}>{textLabel.value}</TextLabel>
		{/if}
	{/each}
</div>

<style lang="scss" global>
	.widget-column {
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
	}

	.widget-row {
		flex: 0 0 auto;
		display: flex;
		min-height: 32px;

		> * {
			--widget-height: 24px;
			margin: calc((24px - var(--widget-height)) / 2 + 4px) 0;
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

		// TODO: Target this in a better way than using the tooltip, which will break if changed, or when localized/translated
		.checkbox-input [title="Preserve Aspect Ratio"] {
			margin-bottom: -32px;
			position: relative;

			&::before,
			&::after {
				content: "";
				pointer-events: none;
				position: absolute;
				left: 8px;
				width: 1px;
				height: 16px;
				background: var(--color-7-middlegray);
			}

			&::before {
				top: calc(-4px - 16px);
			}

			&::after {
				bottom: calc(-4px - 16px);
			}
		}
	}
</style>
