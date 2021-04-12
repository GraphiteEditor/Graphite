interface ColorPickerOptions {
  el: Element;
};

export interface RGBAColor {
  r: number;
  g: number;
  b: number;
  a?: number;
};

export class ColorPicker {
  constructor(options: Partial<ColorPickerOptions> = {}) {

  }

  setColor(color: string | number | RGBAColor) {

  }

  getRGB() {
    return { r: 1, g: 1, b: 1, };
  }

  remove() { }

  onChange(callback: () => void) { }
}

function rgbToHex(color: RGBAColor): string {
  const { r, g, b } = color;
  const hex = [
    "#",
    Math.round(r * 255)
      .toString(16)
      .padStart(2, "0"),
    Math.round(g * 255)
      .toString(16)
      .padStart(2, "0"),
    Math.round(b * 255)
      .toString(16)
      .padStart(2, "0"),
  ];

  return hex.join("").toUpperCase();
}