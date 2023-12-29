<script lang="ts">
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	export let exposed: boolean;
	export let dataType: string;
	export let tooltip: string | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;
</script>

<LayoutRow class="parameter-expose-button">
	<button class:exposed style:--data-type-color={`var(--color-data-${dataType})`} style:--data-type-color-dim={`var(--color-data-${dataType}-dim)`} on:click={action} title={tooltip} tabindex="-1">
		<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
			<path class="interior" d="M0,7.882c0,1.832,1.325,2.63,2.945,1.772L8.785,6.56c1.62-.858,1.62-2.262,0-3.12L2.945.345C1.325-.512,0,.285,0,2.118Z" />
			<path
				class="outline"
				d="M 1.705180287361145 9.999852180480957 L 1.705180287361145 8.999852180480957 C 1.9275803565979 8.999852180480957 2.194530248641968 8.920772552490234 2.476730346679688
				8.771392822265625 L 8.31682014465332 5.67636251449585 C 8.788760185241699 5.426312446594238 9 5.156492233276367 9 5.000002384185791 C 9 4.843512535095215 8.788760185241699
				4.573692321777344 8.316730499267578 4.323602199554443 L 2.477190256118774 1.228852391242981 C 2.194520235061646 1.079232335090637 1.927510380744934 1.000152349472046 1.70503032207489
				1.000152349472046 C 1.091590285301208 1.000152349472046 1.000000357627869 1.700212359428406 1.000000357627869 2.117512464523315 L 1.000000357627869 7.882492542266846 C
				1.000000357627869 8.299762725830078 1.091610312461853 8.999792098999023 1.705130338668823 8.999852180480957 L 1.705180287361145 9.999852180480957 M 1.705027341842651 9.999849319458008
				C 0.7003514766693115 9.999751091003418 0 9.214582443237305 0 7.882492542266846 L 0 2.117512464523315 C 0 0.2850223779678345 1.325000405311584 -0.512467622756958 2.945000410079956
				0.3450223803520203 L 8.785000801086426 3.440012454986572 C 10.40500068664551 4.298342227935791 10.40500068664551 5.701662540435791 8.785000801086426 6.55999231338501 L
				2.945000410079956 9.654982566833496 C 2.502624750137329 9.889138221740723 2.082434415817261 9.999885559082031 1.705027341842651 9.999849319458008 Z"
			/>
		</svg>
	</button>
</LayoutRow>

<style lang="scss" global>
	.parameter-expose-button {
		display: flex;
		align-items: center;
		flex: 0 0 auto;
		max-height: 24px;

		button {
			flex: 0 0 auto;
			width: 8px;
			height: 8px;
			margin: 0;
			padding: 0;
			border: none;
			background: none;
			fill: none;
			stroke: none;

			svg {
				width: 10px;
				height: 10px;
				margin-top: -1px;
				margin-left: -1px;
			}

			&:not(.exposed) {
				&:not(:hover) {
					.outline {
						fill: var(--data-type-color-dim);
					}
				}

				&:hover {
					.outline {
						fill: var(--data-type-color);
					}
				}
			}

			&.exposed {
				&:not(:hover) {
					.interior {
						fill: var(--data-type-color);
					}
				}

				&:hover {
					.outline {
						fill: var(--data-type-color);
					}

					.interior {
						fill: var(--data-type-color-dim);
					}
				}
			}
		}
	}
</style>
