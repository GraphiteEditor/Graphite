<template>
	<div class="checkbox-input">
		<input type="checkbox" :id="`checkbox-${id}`" :checked="checked" @input="(e) => $emit('update:checked', e.target.checked)" />
		<label :for="`checkbox-${id}`">
			<div class="checkbox-box">
				<Icon :icon="icon" />
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

			.icon {
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

			.icon {
				fill: var(--color-f-white);
			}
		}

		&:hover .checkbox-box {
			background: var(--color-accent-hover);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import Icon from "../labels/Icon.vue";

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
	},
	components: { Icon },
});
</script>
