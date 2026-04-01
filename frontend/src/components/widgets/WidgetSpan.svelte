<script lang="ts">
	import { getContext } from "svelte";
	import NodeCatalog from "/src/components/floating-menus/NodeCatalog.svelte";
	import BreadcrumbTrailButtons from "/src/components/widgets/buttons/BreadcrumbTrailButtons.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import ImageButton from "/src/components/widgets/buttons/ImageButton.svelte";
	import ParameterExposeButton from "/src/components/widgets/buttons/ParameterExposeButton.svelte";
	import PopoverButton from "/src/components/widgets/buttons/PopoverButton.svelte";
	import TextButton from "/src/components/widgets/buttons/TextButton.svelte";
	import CheckboxInput from "/src/components/widgets/inputs/CheckboxInput.svelte";
	import ColorInput from "/src/components/widgets/inputs/ColorInput.svelte";
	import CurveInput from "/src/components/widgets/inputs/CurveInput.svelte";
	import DropdownInput from "/src/components/widgets/inputs/DropdownInput.svelte";
	import NumberInput from "/src/components/widgets/inputs/NumberInput.svelte";
	import RadioInput from "/src/components/widgets/inputs/RadioInput.svelte";
	import ReferencePointInput from "/src/components/widgets/inputs/ReferencePointInput.svelte";
	import TextAreaInput from "/src/components/widgets/inputs/TextAreaInput.svelte";
	import TextInput from "/src/components/widgets/inputs/TextInput.svelte";
	import WorkingColorsInput from "/src/components/widgets/inputs/WorkingColorsInput.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import ImageLabel from "/src/components/widgets/labels/ImageLabel.svelte";
	import Separator from "/src/components/widgets/labels/Separator.svelte";
	import ShortcutLabel from "/src/components/widgets/labels/ShortcutLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import { parseFillChoice } from "/src/utility-functions/colors";
	import type { EditorWrapper, LayoutTarget, Widget, WidgetInstance } from "/wrapper/pkg/graphite_wasm_wrapper";

	// Extract the discriminant key names from the Widget tagged enum union (e.g. "TextButton" | "CheckboxInput" | ...)
	type WidgetKind = Widget extends infer T ? (T extends Record<infer K, unknown> ? K & string : never) : never;
	// Extract the props type for a specific widget kind (e.g. WidgetProps<"TextButton"> gives the Wasm-generated TextButton interface)
	type WidgetProps<K extends WidgetKind> = Extract<Widget, Record<K, unknown>>[K];
	// A Widget tagged enum unwrapped into a correlated [kind, props] tuple
	type UnwrappedWidget = { [K in WidgetKind]: [kind: K, props: WidgetProps<K>] }[WidgetKind];

	const editor = getContext<EditorWrapper>("editor");

	export let widgets: WidgetInstance[];
	export let direction: "row" | "column";
	export let layoutTarget: LayoutTarget;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let narrow = false;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");

	function widgetValueCommit(widgetIndex: number, value: unknown) {
		editor.widgetValueCommit(layoutTarget, widgets[widgetIndex].widgetId, value);
	}

	function widgetValueUpdate(widgetIndex: number, value: unknown, resendWidget: boolean) {
		editor.widgetValueUpdate(layoutTarget, widgets[widgetIndex].widgetId, value, resendWidget);
	}

	function widgetValueCommitAndUpdate(widgetIndex: number, value: unknown, resendWidget: boolean) {
		editor.widgetValueCommitAndUpdate(layoutTarget, widgets[widgetIndex].widgetId, value, resendWidget);
	}

	// Extracts the kind and props from a Widget tagged enum, validated against the widget registry.
	// The overload declares the precise correlated return type while the implementation uses broader types.
	function unwrapWidget(widgetInstance: WidgetInstance): UnwrappedWidget | undefined;
	function unwrapWidget(widgetInstance: WidgetInstance) {
		const entry = Object.entries(widgetInstance.widget)[0];
		if (!entry || !(entry[0] in widgetResolvers)) return undefined;
		return entry;
	}

	// Resolves the unwrapped widget through the registry to get its Svelte component and computed props.
	function resolveWidget([kind, widgetProps]: UnwrappedWidget, widgetIndex: number) {
		const config = widgetResolvers[kind];
		return {
			component: config.component,
			props: config.getProps(widgetProps, widgetIndex),
			slot: config.getSlotContent?.(widgetProps),
		};
	}

	// Svelte has no variance-safe base type for component constructors
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	type SvelteComponentAny = any;

	type WidgetConfig<K extends WidgetKind> = {
		component: SvelteComponentAny;
		getProps(props: WidgetProps<K>, widgetIndex: number): Record<string, unknown> | undefined;
		getSlotContent?(props: WidgetProps<K>): string;
	};

	// The union of all individual widget props types (distributed across each WidgetKind member)
	type AnyWidgetProps = { [K in WidgetKind]: WidgetProps<K> }[WidgetKind];

	// Uniform view for runtime lookup — widens the per-kind config types to a single type that
	// accepts any widget props, avoiding the correlated unions problem at the call site
	type WidgetResolver = {
		component: SvelteComponentAny;
		getProps(props: AnyWidgetProps, widgetIndex: number): Record<string, unknown> | undefined;
		getSlotContent?(props: AnyWidgetProps): string;
	};

	// Overload: callers provide the precise mapped type (preserving per-entry type inference).
	// Implementation: receives/returns the widened uniform type (no cast needed).
	// Method syntax bivariance makes WidgetConfig<K> assignable to WidgetResolver in the overload check.
	function createWidgetResolvers(registry: { [K in WidgetKind]: WidgetConfig<K> }): Record<WidgetKind, WidgetResolver>;
	function createWidgetResolvers(registry: Record<WidgetKind, WidgetResolver>): Record<WidgetKind, WidgetResolver> {
		return registry;
	}

	const widgetResolvers = createWidgetResolvers({
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
					value: (e: CustomEvent) => widgetValueCommitAndUpdate(index, e.detail, false),
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
					value: (e: CustomEvent) => widgetValueUpdate(index, e.detail, true),
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
			getProps: ({ value: _, ...rest }) => rest,
			getSlotContent: (props) => props.value,
		},
	});
</script>

<div class={`widget-span ${className} ${extraClasses}`.trim()} class:narrow class:row={direction === "row"} class:column={direction === "column"}>
	{#each widgets as widget, widgetIndex}
		{@const unwrapped = unwrapWidget(widget)}
		{#if unwrapped}
			{@const { component, props, slot } = resolveWidget(unwrapped, widgetIndex)}
			{#if props !== undefined && slot !== undefined}
				<svelte:component this={component} {...props}>{slot}</svelte:component>
			{:else if props !== undefined}
				<svelte:component this={component} {...props} />
			{/if}
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
