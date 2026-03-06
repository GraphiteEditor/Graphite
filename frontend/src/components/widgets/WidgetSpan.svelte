<script lang="ts">
	import { getContext } from "svelte";

	import type { Editor } from "@graphite/editor";
	import type { LayoutTarget, WidgetInstance } from "@graphite/messages";
	import { parseFillChoice } from "@graphite/utility-functions/colors";
	import { debouncer } from "@graphite/utility-functions/debounce";
	import type { WidgetSpanColumn, WidgetSpanRow, WidgetKind } from "@graphite/utility-functions/widgets";
	import { isWidgetSpanColumn, isWidgetSpanRow } from "@graphite/utility-functions/widgets";

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
		if (isWidgetSpanRow(widgetData)) widgets = widgetData.row.rowWidgets;
		else if (isWidgetSpanColumn(widgetData)) widgets = widgetData.column.columnWidgets;
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
	function exclude(props: Record<string, unknown>, additional: string[]): Record<string, any> {
		const exclusions = new Set(additional);
		return Object.fromEntries(Object.entries(props).filter(([key]) => !exclusions.has(key)));
	}

	// Extracts the kind name and props from a Widget tagged enum (e.g. `{ TextButton: { label: "..." } }` -> `["TextButton", { label: "..." }]`)
	function unwrapWidget(widgetInstance: WidgetInstance): [WidgetKind, Record<string, unknown>] {
		const entries = Object.entries(widgetInstance.widget);
		return entries[0] as [WidgetKind, Record<string, unknown>];
	}

	type WidgetConfig = {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		component: any;
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		getProps(props: Record<string, unknown>, widgetIndex: number): Record<string, any> | undefined;
		getSlotContent?(props: Record<string, unknown>): string;
	};

	const widgetRegistry: Record<WidgetKind, WidgetConfig> = {
		CheckboxInput: {
			component: CheckboxInput,
			getProps: (props, index) => ({
				...props,
				$$events: { checked: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		ColorInput: {
			component: ColorInput,
			getProps: (props, index) => ({
				...props,
				value: parseFillChoice(props.value),
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
				...props,
				$$events: {
					value: (e: CustomEvent) => debouncer((value: unknown) => widgetValueCommitAndUpdate(index, value, false), { debounceTime: 120 }).debounceUpdateValue(e.detail),
				},
			}),
		},
		DropdownInput: {
			component: DropdownInput,
			getProps: (props, index) => ({
				...props,
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
				...props,
				action: () => widgetValueCommitAndUpdate(index, undefined, true),
			}),
		},
		IconButton: {
			component: IconButton,
			getProps: (props, index) => ({
				...props,
				action: () => widgetValueCommitAndUpdate(index, undefined, true),
			}),
		},
		IconLabel: {
			component: IconLabel,
			getProps: (props) => ({ ...props }),
		},
		ShortcutLabel: {
			component: ShortcutLabel,
			getProps: (props) => {
				if (!props.shortcut) return undefined;
				return { ...props };
			},
		},
		ImageLabel: {
			component: ImageLabel,
			getProps: (props) => ({ ...props }),
		},
		ImageButton: {
			component: ImageButton,
			getProps: (props, index) => ({
				...props,
				action: () => widgetValueCommitAndUpdate(index, undefined, true),
			}),
		},
		NodeCatalog: {
			component: NodeCatalog,
			getProps: (props, index) => ({
				...props,
				$$events: { selectNodeType: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, false) },
			}),
		},
		NumberInput: {
			component: NumberInput,
			getProps: (props, index) => ({
				...props,
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
				...props,
				$$events: { value: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		PopoverButton: {
			component: PopoverButton,
			getProps: (props) => ({
				...props,
				layoutTarget,
			}),
		},
		RadioInput: {
			component: RadioInput,
			getProps: (props, index) => ({
				...props,
				$$events: { selectedIndex: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		Separator: {
			component: Separator,
			getProps: (props) => ({ ...props }),
		},
		WorkingColorsInput: {
			component: WorkingColorsInput,
			getProps: (props) => ({ ...props }),
		},
		TextAreaInput: {
			component: TextAreaInput,
			getProps: (props, index) => ({
				...props,
				$$events: { commitText: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, false) },
			}),
		},
		TextButton: {
			component: TextButton,
			getProps: (props, index) => ({
				...props,
				action: () => widgetValueCommitAndUpdate(index, [], true),
				$$events: { selectedEntryValuePath: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, false) },
			}),
		},
		BreadcrumbTrailButtons: {
			component: BreadcrumbTrailButtons,
			getProps: (props, index) => ({
				...props,
				action: (breadcrumbIndex: number) => widgetValueCommitAndUpdate(index, breadcrumbIndex, true),
			}),
		},
		TextInput: {
			component: TextInput,
			getProps: (props, index) => ({
				...props,
				$$events: { commitText: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, true) },
			}),
		},
		TextLabel: {
			component: TextLabel,
			getProps: (props) => exclude(props, ["value"]),
			getSlotContent: (props) => props.value as string,
		},
	};
</script>

<div class={`widget-span ${className} ${extraClasses}`.trim()} class:narrow class:row={direction === "row"} class:column={direction === "column"}>
	{#each widgets as widget, widgetIndex}
		{@const [kind, widgetProps] = unwrapWidget(widget)}
		{@const config = widgetRegistry[kind]}
		{@const props = config?.getProps(widgetProps, widgetIndex)}
		{@const slot = config?.getSlotContent?.(widgetProps)}
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
