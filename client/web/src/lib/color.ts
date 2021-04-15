import { Hex2String, Hex2RGB, RGB2Floats, Floats2HSV, RGB2Hex, Floats2RGB, HSV2Floats, HSV, RGB, RGBFloats } from "./utils";

export class Color {
	floats: RGBFloats = {
		r: 0,
		g: 0,
		b: 0,
		a: 1,
	};

	rgb: RGB = {
		r: 0,
		g: 0,
		b: 0,
		a: 1,
	};

	hsv: HSV = {
		h: 0,
		s: 0,
		v: 0,
		a: 1,
	};

	hex = 0x000000;

	hexString = "000000";

	get alpha() {
		return this.rgb.a;
	}

	set alpha(value: number) {
		this.rgb.a = value;
		this.hsv.a = value;
		this.floats.a = value;
	}

	static fromHex(hex: string | number, alpha = 1) {
		const c = new Color();
		c.setHex(hex);
		c.alpha = alpha;
		return c;
	}

	static fromRGB(r: number, g: number, b: number, alpha = 1) {
		const c = new Color();
		c.setRGB(r, g, b);
		c.alpha = alpha;
		return c;
	}

	static fromFloats(r: number, g: number, b: number, alpha = 1) {
		const c = new Color();
		c.setFloats(r, g, b);
		c.alpha = alpha;
		return c;
	}

	static fromHSV(h: number, s: number, v: number, alpha = 1) {
		const c = new Color();
		c.setHSV(h, s, v);
		c.alpha = alpha;
		return c;
	}

	clone() {
		return Color.copy(this, new Color());
	}

	setHex(hex: string | number) {
		if (typeof hex === "string") {
			this.hex = parseInt(hex, 16);
			this.hexString = hex;
		} else {
			this.hex = hex;
			this.hexString = Hex2String(hex);
		}

		Hex2RGB(hex, this.rgb);
		RGB2Floats(this.rgb, this.floats);
		Floats2HSV(this.floats, this.hsv);
	}

	setRGB(r: number, g: number, b: number) {
		this.rgb.r = r;
		this.rgb.g = g;
		this.rgb.b = b;
		RGB2Floats(this.rgb, this.floats);
		Floats2HSV(this.floats, this.hsv);
		this.hex = RGB2Hex(this.rgb);
		this.hexString = Hex2String(this.hex);
	}

	setFloats(r: number, g: number, b: number) {
		this.floats.r = r;
		this.floats.g = g;
		this.floats.b = b;
		Floats2RGB(this.floats, this.rgb);
		Floats2HSV(this.floats, this.hsv);
		this.hex = RGB2Hex(this.rgb);
		this.hexString = Hex2String(this.hex);
	}

	setHSV(h: number, s: number, v: number) {
		this.hsv.h = h;
		this.hsv.s = s;
		this.hsv.v = v;
		HSV2Floats(this.hsv, this.floats);
		Floats2RGB(this.floats, this.rgb);
		this.hex = RGB2Hex(this.rgb);
		this.hexString = Hex2String(this.hex);
	}

	setAlpha(alpha: number) {
		this.alpha = alpha;
	}

	static copy(from: Color, to: Color) {
		to.setHex(from.hex);
		return to;
	}

	static toCSS(color: Color, type: "rgb" | "rgba" | "hex" = "rgb") {
		switch (type) {
			case "hex": {
				return `#${color.hexString}`;
			}
			case "rgba": {
				return `rgba(${color.rgb.r}, ${color.rgb.g}, ${color.rgb.b}, ${color.rgb.a})`;
			}
			case "rgb":
			default: {
				return `rgb(${color.rgb.r}, ${color.rgb.g}, ${color.rgb.b})`;
			}
		}
	}
}
