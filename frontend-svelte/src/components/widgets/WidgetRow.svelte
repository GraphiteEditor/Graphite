<script lang="ts">
	import { debouncer } from "@/utility-functions/debounce";
	import type { Widget } from "@/wasm-communication/messages";
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
	import { type Editor } from "@/wasm-communication/editor";

	const SUFFIX_WIDGETS = ["PopoverButton"];

	const editor = getContext<Editor>("editor");

	export let widgetData: WidgetColumn | WidgetRow;
	export let layoutTarget: any;

	let open = false;

	$: direction = watchDirection(widgetData);
	$: widgets = watchWidgets(widgetData);
	$: widgetsAndNextSiblingIsSuffix = watchWidgetsAndNextSiblingIsSuffix(widgets);

	function watchDirection(widgetData: WidgetColumn): "column" | "row" | "ERROR" {
		if (isWidgetColumn(widgetData)) return "column";
		if (isWidgetRow(widgetData)) return "row";
		return "ERROR";
	}

	function watchWidgets(widgetData: WidgetColumn): Widget[] {
		let widgets: Widget[] = [];
		if (isWidgetColumn(widgetData)) widgets = widgetData.columnWidgets;
		else if (isWidgetRow(widgetData)) widgets = widgetData.rowWidgets;
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
	function excludeValue(props: Record<string, any>): Record<string, unknown> {
		const { value: _, ...rest } = props;
		return rest;
	}
</script>

<!-- TODO: Refactor this component to use `<component :is="" v-bind="attributesObject"></component>` to avoid all the separate components with `v-if` -->
<!-- TODO: Also rename this component, and probably move the `widget-${direction}` wrapper to be part of `WidgetLayout.svelte` as part of its refactor -->

<div class={`widget-${direction}`}>
	{#each widgetsAndNextSiblingIsSuffix as [component, nextIsSuffix], index (index)}
		{#if component.props.kind === "CheckboxInput"}
			<CheckboxInput {...component.props} on:checked={(value) => updateLayout(index, value)} />
		{/if}
		{#if component.props.kind === "ColorInput"}
			<ColorInput {...component.props} bind:open on:value={(value) => updateLayout(index, value)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{#if component.props.kind === "DropdownInput"}
			<DropdownInput {...component.props} bind:open on:selectedIndex={(value) => updateLayout(index, value)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{#if component.props.kind === "FontInput"}
			<FontInput {...component.props} bind:open on:changeFont={({ detail }) => updateLayout(index, detail)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{#if component.props.kind === "ParameterExposeButton"}
			<ParameterExposeButton {...component.props} action={() => updateLayout(index, undefined)} />
		{/if}
		{#if component.props.kind === "IconButton"}
			<IconButton {...component.props} action={() => updateLayout(index, undefined)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{#if component.props.kind === "IconLabel"}
			<IconLabel {...component.props} />
		{/if}
		{#if component.props.kind === "LayerReferenceInput"}
			<LayerReferenceInput {...component.props} on:value={(value) => updateLayout(index, value)} />
		{/if}
		{#if component.props.kind === "NumberInput"}
			<NumberInput
				{...component.props}
				on:value={debouncer((value) => updateLayout(index, value)).updateValue}
				incrementCallbackIncrease={() => updateLayout(index, "Increment")}
				incrementCallbackDecrease={() => updateLayout(index, "Decrement")}
				sharpRightCorners={nextIsSuffix}
			/>
		{/if}
		{#if component.props.kind === "OptionalInput"}
			<OptionalInput {...component.props} on:checked={(value) => updateLayout(index, value)} />
		{/if}
		{#if component.props.kind === "PivotAssist"}
			<PivotAssist {...component.props} on:position={(value) => updateLayout(index, value)} />
		{/if}
		{#if component.props.kind === "PopoverButton"}
			<PopoverButton {...component.props}>
				<TextLabel bold={true}>{component.props.header}</TextLabel>
				<TextLabel multiline={true}>{component.props.text}</TextLabel>
			</PopoverButton>
		{/if}
		{#if component.props.kind === "RadioInput"}
			<RadioInput {...component.props} on:selectedIndex={(value) => updateLayout(index, value)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{#if component.props.kind === "Separator"}
			<Separator {...component.props} />
		{/if}
		{#if component.props.kind === "SwatchPairInput"}
			<SwatchPairInput {...component.props} />
		{/if}
		{#if component.props.kind === "TextAreaInput"}
			<TextAreaInput {...component.props} on:commitText={({ detail }) => updateLayout(index, detail)} />
		{/if}
		{#if component.props.kind === "TextButton"}
			<TextButton {...component.props} action={() => updateLayout(index, undefined)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{#if component.props.kind === "BreadcrumbTrailButtons"}
			<BreadcrumbTrailButtons {...component.props} action={(index) => updateLayout(index, index)} />
		{/if}
		{#if component.props.kind === "TextInput"}
			<TextInput {...component.props} on:commitText={({ detail }) => updateLayout(index, detail)} sharpRightCorners={nextIsSuffix} />
		{/if}
		{#if component.props.kind === "TextLabel"}
			<TextLabel {...excludeValue(component.props)}>{component.props.value}</TextLabel>
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
