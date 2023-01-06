<script lang="ts">
  let className = "";
  export { className as class };
  export let classes: Record<string, boolean> = {};
  let styleName = "";
  export { styleName as style };
  export let styles: Record<string, string | number> = {};
  export let tooltip: string | undefined = undefined;
  export let scrollableX: boolean = false;
  export let scrollableY: boolean = false;
  export let click: ((e: MouseEvent) => void) | undefined = undefined;
  export let pointerdown: ((e: PointerEvent) => void) | undefined = undefined;

  $: extraClasses = Object.entries(classes)
    .flatMap((classAndState) => (classAndState[1] ? [classAndState[0]] : []))
    .join(" ");
  $: extraStyles = Object.entries(styles)
    .map((styleAndValue) => `${styleAndValue[0]}: ${styleAndValue[1]};`)
    .join(" ");
</script>

<div
  class={`layout-col ${className} ${extraClasses}`.trim()}
  class:scrollable-x={scrollableX}
  class:scrollable-y={scrollableY}
  style={`${styleName} ${extraStyles}`.trim()}
  title={tooltip}
  on:click={click}
  on:pointerdown={pointerdown}
  {...$$restProps}
>
  <slot />
</div>

<style lang="scss" global>
  .layout-col {
    display: flex;
    flex-direction: column;
    flex-grow: 1;
  }
</style>
