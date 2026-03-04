<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { LayoutTarget, WidgetInstance, WidgetSpanColumn, WidgetSpanRow } from "@graphite/messages";
	import { isWidgetSpanColumn, isWidgetSpanRow } from "@graphite/messages";
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

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	function exclude(props: Record<string, any>, additional?: string[]): Record<string, any> {
		const exclusions = new Set(["kind", ...(additional || [])]);
		return Object.fromEntries(Object.entries(props).filter(([key]) => !exclusions.has(key)));
	}

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	type WidgetConfig = { component: any; getProps: (props: any, widgetIndex: number) => Record<string, any> | undefined; getSlotContent?: (props: any) => string };

	const widgetRegistry: Record<string, WidgetConfig> = {
		CheckboxInput: {
			component: CheckboxInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: { checked: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		ColorInput: {
			component: ColorInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: {
					value: (e: CustomEvent) => widgetValueUpdate(index, e.detail, false),
					startHistoryTransaction: () => widgetValueCommit(index, props.value),
				},
			}),
		},
		CurveInput: {
			// TODO: CurvesInput is currently unused
			component: CurveInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: {
					value: (e: CustomEvent) => debouncer((value: unknown) => widgetValueCommitAndUpdate(index, value, false), { debounceTime: 120 }).debounceUpdateValue(e.detail),
				},
			}),
		},
		DropdownInput: {
			component: DropdownInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: {
					hoverInEntry: (e: CustomEvent) => widgetValueUpdate(index, e.detail, false),
					hoverOutEntry: (e: CustomEvent) => widgetValueUpdate(index, e.detail, false),
					selectedIndex: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true),
				},
			}),
		},
		ParameterExposeButton: {
			component: ParameterExposeButton,
			getProps: (props, index) => ({
				...exclude(props),
				action: () => widgetValueCommitAndUpdate(index, undefined, true),
			}),
		},
		IconButton: {
			component: IconButton,
			getProps: (props, index) => ({
				...exclude(props),
				action: () => widgetValueCommitAndUpdate(index, undefined, true),
			}),
		},
		IconLabel: {
			component: IconLabel,
			getProps: (props) => exclude(props),
		},
		ShortcutLabel: {
			component: ShortcutLabel,
			getProps: (props) => {
				if (!props.shortcut) return undefined;
				return exclude(props);
			},
		},
		ImageLabel: {
			component: ImageLabel,
			getProps: (props) => exclude(props),
		},
		ImageButton: {
			component: ImageButton,
			getProps: (props, index) => ({
				...exclude(props),
				action: () => widgetValueCommitAndUpdate(index, undefined, true),
			}),
		},
		NodeCatalog: {
			component: NodeCatalog,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: { selectNodeType: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, false) },
			}),
		},
		NumberInput: {
			component: NumberInput,
			getProps: (props, index) => ({
				...exclude(props),
				incrementCallbackIncrease: () => widgetValueCommitAndUpdate(index, "Increment", false),
				incrementCallbackDecrease: () => widgetValueCommitAndUpdate(index, "Decrement", false),
				$$events: {
					value: (e: CustomEvent) => debouncer((value: unknown) => widgetValueUpdate(index, value, true)).debounceUpdateValue(e.detail),
					startHistoryTransaction: () => widgetValueCommit(index, props.value),
				},
			}),
		},
		ReferencePointInput: {
			component: ReferencePointInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: { value: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		PopoverButton: {
			component: PopoverButton,
			getProps: (props) => ({
				...exclude(props),
				layoutTarget,
			}),
		},
		RadioInput: {
			component: RadioInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: { selectedIndex: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		Separator: {
			component: Separator,
			getProps: (props) => exclude(props),
		},
		WorkingColorsInput: {
			component: WorkingColorsInput,
			getProps: (props) => exclude(props),
		},
		TextAreaInput: {
			component: TextAreaInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: { commitText: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, false) },
			}),
		},
		TextButton: {
			component: TextButton,
			getProps: (props, index) => ({
				...exclude(props),
				action: () => widgetValueCommitAndUpdate(index, [], true),
				$$events: { selectedEntryValuePath: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, false) },
			}),
		},
		BreadcrumbTrailButtons: {
			component: BreadcrumbTrailButtons,
			getProps: (props, index) => ({
				...exclude(props),
				action: (breadcrumbIndex: number) => widgetValueCommitAndUpdate(index, breadcrumbIndex, true),
			}),
		},
		TextInput: {
			component: TextInput,
			getProps: (props, index) => ({
				...exclude(props),
				$$events: { commitText: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		TextLabel: {
			component: TextLabel,
			getProps: (props) => exclude(props, ["value"]),
			getSlotContent: (props) => props.value,
		},
	};
</script>

<div class={`widget-span ${className} ${extraClasses}`.trim()} class:narrow class:row={direction === "row"} class:column={direction === "column"}>
	{#each widgets as widget, widgetIndex}
		{@const config = widgetRegistry[widget.props.kind]}
		{@const props = config?.getProps(widget.props, widgetIndex)}
		{@const slot = config?.getSlotContent?.(widget.props)}
		{#if props !== undefined && slot !== undefined}
			<svelte:component this={config.component} {...props}>{slot}</svelte:component>
		{:else if props !== undefined}
			<svelte:component this={config.component} {...props} />
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
		min-height: var(--row-height);
		--row-height: 32px;

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
