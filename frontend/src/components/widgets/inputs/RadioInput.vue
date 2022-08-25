<template>
	<LayoutRow class="radio-input">
		<button :class="{ active: index === selectedIndex }" v-for="(entry, index) in entries" :key="index" @click="() => handleEntryClick(entry)" :title="entry.tooltip">
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
		padding: 0 4px;
		outline: none;
		border: none;
		display: flex;
		align-items: center;

		&:hover {
			background: var(--color-6-lowergray);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
			}
		}

		&.active {
			background: var(--color-accent);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
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
	}

	&.combined-before button:first-of-type,
	&.combined-after button:last-of-type {
		border-radius: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { RadioEntries, RadioEntryData } from "@/wasm-communication/messages";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	emits: ["update:selectedIndex"],
	props: {
		entries: { type: Array as PropType<RadioEntries>, required: true },
		selectedIndex: { type: Number as PropType<number>, required: true },
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
		TextLabel,
		LayoutRow,
	},
});
</script>
