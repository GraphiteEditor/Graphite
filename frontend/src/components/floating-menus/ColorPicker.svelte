<script lang="ts">
	import { createEventDispatcher, getContext } from "svelte";
	import FloatingMenu from "/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { ColorPickerStore } from "/src/stores/color-picker";
	import type { EditorWrapper, FillChoice, MenuDirection } from "/wrapper/pkg/graphite_wasm_wrapper";

	const dispatch = createEventDispatcher<{ colorOrGradient: FillChoice; startHistoryTransaction: undefined; commitHistoryTransaction: undefined }>();

	const editor = getContext<EditorWrapper>("editor");
	const colorPickerStore = getContext<ColorPickerStore>("colorPicker");

	export let colorOrGradient: FillChoice;
	export let allowNone = false;
	// export let allowTransparency = false; // TODO: Implement
	export let disabled = false;
	export let direction: MenuDirection = "Bottom";
	// TODO: See if this should be made to follow the pattern of DropdownInput.svelte so this could be removed
	export let open: boolean;

	// Suppress the FloatingMenu's stray-close while the user is dragging one of the visual H/S/V/A pickers.
	$: strayCloses = !$colorPickerStore.isDragging;
	let self: FloatingMenu | undefined;

	// Open/close lifecycle: when `open` flips, register/clear the global callbacks (so events route to *this* instance)
	// and tell the Rust handler to (re)initialize its state from the current `colorOrGradient`.
	let lastOpen = false;
	$: handleOpenChange(open);

	function handleOpenChange(isOpen: boolean) {
		if (isOpen && !lastOpen) {
			colorPickerStore.setCallbacks({
				onColorChanged: (value) => dispatch("colorOrGradient", value),
				onStartTransaction: () => dispatch("startHistoryTransaction"),
				onCommitTransaction: () => dispatch("commitHistoryTransaction"),
			});
			editor.openColorPicker(colorOrGradient, allowNone, disabled);
			// Auto-select the hex color code text input. Deferred so the layout has time to render after the picker opens.
			setTimeout(() => {
				const hexInput = self?.div()?.querySelector(".text-input input");
				if (hexInput instanceof HTMLInputElement) hexInput.select();
			}, 0);
		} else if (!isOpen && lastOpen) {
			colorPickerStore.clearCallbacks();
			editor.closeColorPicker();
		}
		lastOpen = isOpen;
	}

	export function div(): HTMLDivElement | undefined {
		return self?.div();
	}
</script>

<FloatingMenu class="color-picker" classes={{ disabled }} {open} on:open {strayCloses} escapeCloses={strayCloses} {direction} type="Popover" bind:this={self}>
	<LayoutRow>
		<LayoutCol class="pickers-and-gradient">
			<WidgetLayout layout={$colorPickerStore.pickersAndGradient} layoutTarget="ColorPickerPickersAndGradient" />
		</LayoutCol>
		<LayoutCol class="details">
			<WidgetLayout layout={$colorPickerStore.details} layoutTarget="ColorPickerDetails" />
		</LayoutCol>
	</LayoutRow>
</FloatingMenu>

<style lang="scss">
	.color-picker {
		--widget-height: 24px;

		.pickers-and-gradient {
			.visual-color-pickers-input {
				margin: 0;
			}

			.widget-span {
				--row-height: 24px;

				&:has(.spectrum-input) {
					margin-top: 16px;

					.spectrum-input {
						flex: 1 1 100%;
					}

					.number-input {
						margin-left: 8px;
						min-width: 0;
						width: calc(24px + 8px + 24px);
						flex: 0 0 auto;
					}
				}
			}
		}

		.details {
			margin-left: 16px;
			width: 200px;
			gap: 8px;

			> .widget-span {
				--row-height: 24px;
				flex: 0 0 auto;

				&:last-child {
					margin-top: auto;
				}

				> .text-label {
					// TODO: Use a table or grid layout for this width to match the widest label. Hard-coding it won't work when we add translation/localization.
					flex: 0 0 34px;
					line-height: 24px;
				}
			}
		}
	}
</style>
