<template>
	<div class="checkbox-input" :class="{ 'outline-style': outlineStyle }">
		<input type="checkbox" :id="`checkbox-input-${id}`" :checked="checked" @input="(e) => $emit('update:checked', e.target.checked)" />
		<label :for="`checkbox-input-${id}`">
			<div class="checkbox-box">
				<IconLabel :icon="icon" />
			</div>
		</label>
	</div>
</template>

<style lang="scss">
.checkbox-input {
	display: inline-block;

	input {
		display: none;
	}

	label {
		display: block;

		.checkbox-box {
			display: block;
			background: var(--color-e-nearwhite);
			padding: 2px;
			border-radius: 2px;

			.icon-label {
				fill: var(--color-2-mildblack);
			}
		}

		&:hover .checkbox-box {
			background: var(--color-f-white);
		}
	}

	input:checked + label {
		.checkbox-box {
			background: var(--color-accent);

			.icon-label {
				fill: var(--color-f-white);
			}
		}

		&:hover .checkbox-box {
			background: var(--color-accent-hover);
		}
	}

	&.outline-style label {
		.checkbox-box {
			border: 1px solid var(--color-e-nearwhite);
			padding: 1px;
			background: none;

			svg {
				display: none;
			}
		}

		&:hover .checkbox-box {
			border: 1px solid var(--color-f-white);
		}
	}

	&.outline-style input:checked + label {
		.checkbox-box {
			background: none;

			svg {
				display: block;
				fill: var(--color-e-nearwhite);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	data() {
		return {
			id: `${Math.random()}`.substring(2),
		};
	},
	methods: {
		isChecked() {
			return this.checked;
		},
	},
	props: {
		checked: { type: Boolean, required: true },
		icon: { type: String, default: "Checkmark" },
		outlineStyle: { type: Boolean, default: false },
	},
	components: { IconLabel },
});
</script>
