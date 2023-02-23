<template>
	<LayoutRow class="radio-input" :class="{ disabled }">
		<button
			:class="{ active: index === selectedIndex, disabled, 'sharp-right-corners': index === entries.length - 1 && sharpRightCorners }"
			v-for="(entry, index) in entries"
			:key="index"
			@click="() => handleEntryClick(entry)"
			:title="entry.tooltip"
			:tabindex="index === selectedIndex ? -1 : 0"
			:disabled="disabled"
		>
			<IconLabel v-if="entry.icon" :icon="entry.icon" />
			<TextLabel v-if="entry.label">{{ entry.label }}</TextLabel>
		</button>
	</LayoutRow>
</template>

<style lang="scss">
.radio-input {
	button {
		background: var(--color-5-dullgray);
		fill: var(--color-e-nearwhite);
		height: 24px;
		margin: 0;
		padding: 0 4px;
		border: none;
		display: flex;
		align-items: center;
		justify-content: center;

		&:hover {
			background: var(--color-6-lowergray);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
			}
		}

		&.active {
			background: var(--color-e-nearwhite);
			color: var(--color-2-mildblack);

			svg {
				fill: var(--color-2-mildblack);
			}
		}

		&.disabled {
			background: var(--color-4-dimgray);
			color: var(--color-8-uppergray);

			svg {
				fill: var(--color-8-uppergray);
			}

			&.active {
				background: var(--color-8-uppergray);
				color: var(--color-2-mildblack);

				svg {
					fill: var(--color-2-mildblack);
				}
			}
		}

		& + button {
			margin-left: 1px;
		}

		&:first-of-type {
			border-radius: 2px 0 0 2px;
		}

		&:last-of-type {
			border-radius: 0 2px 2px 0;
		}
	}

	.text-label {
		margin: 0 4px;
		overflow: hidden;
	}

	&.combined-before button:first-of-type,
	&.combined-after button:last-of-type {
		border-radius: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type RadioEntries, type RadioEntryData } from "@/wasm-communication/messages";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	emits: ["update:selectedIndex"],
	props: {
		entries: { type: Array as PropType<RadioEntries>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		selectedIndex: { type: Number as PropType<number>, required: true },
		sharpRightCorners: { type: Boolean as PropType<boolean>, default: false },
	},
	methods: {
		handleEntryClick(radioEntryData: RadioEntryData) {
			const index = this.entries.indexOf(radioEntryData);
			this.$emit("update:selectedIndex", index);

			radioEntryData.action?.();
		},
	},
	components: {
		IconLabel,
		LayoutRow,
		TextLabel,
	},
});
</script>
