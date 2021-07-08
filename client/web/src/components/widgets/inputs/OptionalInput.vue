<template>
	<div class="optional-input">
		<CheckboxInput :checked="checked" @input="(e) => $emit('update:checked', e.target.checked)" :icon="icon" />
	</div>
</template>

<style lang="scss">
.optional-input {
	label {
		display: flex;
		align-items: center;
		white-space: nowrap;
		justify-content: center;
		width: 24px;
		height: 24px;
		border-radius: 2px 0 0 2px;
		background: var(--color-5-dullgray);

		.checkbox-box {
			background: var(--color-e-nearwhite);

			.icon {
				fill: var(--color-5-dullgray);
			}
		}

		&:hover {
			background: var(--color-6-lowergray);

			.checkbox-box {
				background: var(--color-f-white);

				.icon {
					fill: var(--color-6-lowergray);
				}
			}
		}
	}

	input:checked + label {
		background: var(--color-accent);

		.checkbox-box,
		.checkbox-box:hover {
			background: var(--color-f-white);

			.icon {
				fill: var(--color-accent);
			}
		}

		&:hover {
			background: var(--color-accent-hover);

			.checkbox-box,
			.checkbox-box:hover {
				.icon {
					fill: var(--color-accent-hover);
				}
			}
		}
	}

}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import CheckboxInput from "./CheckboxInput.vue";

export default defineComponent({
	props: {
		checked: { type: Boolean, required: true },
		icon: { type: String, default: "Checkmark" },
	},
	components: {
		CheckboxInput,
	},
});
</script>
