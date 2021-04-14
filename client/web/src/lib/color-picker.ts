import { Color } from "./color"
import { clamp } from "./utils"

interface ColorPickerOptions {
  width: number
  height: number
  cssClassPrefix: string
};

export class ColorPicker {
  // DOM Elements
  private $picker!: HTMLDivElement
  private $hue!: HTMLDivElement
  private $saturation!: HTMLDivElement
  private $hueSelector!: HTMLDivElement
  private $saturationSelector!: HTMLDivElement
  private $brightness!: HTMLDivElement

  // On Change listeners
  private onchange = new Set<(color: Color) => void>()

  private hueColor = Color.fromHSV(1, 1, 1)
  private color = Color.fromRGB(255, 255, 255)
  private size: [number, number] = [256, 256]
  private state: "idle" | "move_hue" | "move_saturation" = "idle"
  private rects = {
    saturation: {
      width: 0,
      height: 0,
      left: 0,
      top: 0,
    },
    hue: {
      width: 0,
      height: 0,
      left: 0,
      top: 0,
    },
  }

  private hue = 0
  private huePosition = 0
  private colorPosition: [number, number] = [0, 0]

  constructor(options: Partial<ColorPickerOptions & { el: Element }> = {}) {
    const o: ColorPickerOptions = {
      width: 256,
      height: 256,
      cssClassPrefix: "color-picker",
      ...options
    }

    this._init(o)

    if (options.el) {
      options.el.append(this.$picker)
    }

    this.setSize(o.width, o.height)
  }

  private _init(options: ColorPickerOptions) {
    const hue = document.createElement("div")
    hue.classList.add(`${options.cssClassPrefix}__hue`)
    const hueSelector = document.createElement("div")
    hueSelector.classList.add(`${options.cssClassPrefix}__hue-selector`)
    const saturation = document.createElement("div")
    saturation.classList.add(`${options.cssClassPrefix}__saturation`)
    const saturationSelector = document.createElement("div")
    saturationSelector.classList.add(`${options.cssClassPrefix}__saturation-selector`)
    const brightness = document.createElement("div")
    brightness.classList.add(`${options.cssClassPrefix}__brightness`)
    const picker = document.createElement("div")
    picker.classList.add(`${options.cssClassPrefix}__picker`)

    hue.append(hueSelector)
    saturation.append(saturationSelector, brightness)
    picker.append(saturation, hue)

    hue.addEventListener("mousedown", this._onMouse)
    hue.addEventListener("touchstart", this._onTouch)
    saturation.addEventListener("mousedown", this._onMouse)
    saturation.addEventListener("touchstart", this._onTouch)

    this.$picker = picker
    this.$saturation = saturation
    this.$saturationSelector = saturationSelector
    this.$hue = hue
    this.$hueSelector = hueSelector
    this.$brightness = brightness
  }

  setSize(width: number, height: number) {
    this.size[0] = width
    this.size[1] = height
    this.$saturation.style.width = width + "px"
    this.$saturation.style.height = height + "px"
    this.$hue.style.width = 20 + "px"
    this.$hue.style.height = height + "px"
  }

  setHex(hex: string | number) {
    this.color.setHex(hex)
    this._setColor()
  }

  setRGB(r: number, g: number, b: number) {
    this.color.setRGB(r, g, b)
    this._setColor()
  }

  setFloats(r: number, g: number, b: number) {
    this.color.setFloats(r, g, b)
    this._setColor()
  }

  setHSV(h: number, s: number, v: number) {
    this.color.setHSV(h, s, v)
    this._setColor()
  }

  setColor(color: Color) {
    Color.copy(color, this.color)
    this._setColor()
  }

  private _setColor() {
    const hsv = this.color.hsv

    this._updateRects()

    this.hue = hsv.h
    this._setSaturationPosition(
      hsv.s * this.rects.saturation.width,
      (1 - hsv.v) * this.rects.saturation.height,
    )
    this._setHuePosition((1 - hsv.h) * this.rects.hue.height)

    this._updateHue()
  }

  getHex() {
    return this.color.hex
  }

  getHexString() {
    return this.color.hexString
  }

  getRGB() {
    return this.color.rgb
  }

  getFloats() {
    return this.color.floats
  }

  getHSV() {
    return this.color.hsv
  }

  getColor() {
    return this.color
  }

  onChange(callback: (color: Color) => void) {
    this.onchange.add(callback)
  }

  dispose() {
    this.onchange.clear()
  }

  private _setSaturationPosition(x: number, y: number) {
    this.colorPosition[0] = clamp(x, 0, this.rects.saturation.width)
    this.colorPosition[1] = clamp(y, 0, this.rects.saturation.height)
    this.$saturationSelector.style.transform = `matrix(1, 0, 0, 1, ${this.colorPosition[0]}, ${this.colorPosition[1]})`
  }

  private _updateSaturationFromPosition() {
    this.color.setHSV(
      this.hue,
      this.colorPosition[0] / this.rects.saturation.width,
      1 - (this.colorPosition[1] / this.rects.saturation.height),
    )
    this._updateSaturation()
  }

  private _updateSaturation() {
    this.$saturationSelector.style.background = `#${this.getHexString()}`
    this._change()
  }

  private _setHuePosition(y: number) {
    this.huePosition = clamp(y, 0, this.rects.hue.height)
    this.$hueSelector.style.transform = `matrix(1, 0, 0, 1, 0, ${this.huePosition})`
  }

  private _updateHueFromPosition() {
    const hsv = this.color.hsv
    this.hue = 1 - (this.huePosition / this.rects.hue.height)
    this.color.setHSV(this.hue, hsv.s, hsv.v)
    this._updateHue()
  }

  private _updateHue() {
    this.hueColor.setHSV(this.hue, 1, 1)
    this.$saturation.style.background = `linear-gradient(to right, #fff, ${Color.toCSS(this.hueColor)})`
    this._updateSaturation()
  }

  private _change() {
    for (const cb of this.onchange) {
      cb(this.color)
    }
  }

  private _updateRects() {
    const saturation = this.$saturation.getBoundingClientRect()
    this.rects.saturation.width = saturation.width
    this.rects.saturation.height = saturation.height
    this.rects.saturation.left = saturation.left
    this.rects.saturation.top = saturation.top

    const hue = this.$hue.getBoundingClientRect()
    this.rects.hue.width = hue.width
    this.rects.hue.height = hue.height
    this.rects.hue.left = hue.left
    this.rects.hue.top = hue.top
  }

  private _onMouse = (e: MouseEvent) => {
    switch (e.type) {
      case "mousedown":
        {
          window.addEventListener("mousemove", this._onMouse)
          window.addEventListener("mouseup", this._onMouse)
          this._onPointer("down", e.currentTarget, e.clientX, e.clientY)
          break
        }
      case "mouseup":
        {
          window.removeEventListener("mousemove", this._onMouse)
          window.removeEventListener("mouseup", this._onMouse)
          this._onPointer("up", e.currentTarget, e.clientX, e.clientY)
          break
        }
      case "mousemove":
        {
          this._onPointer("move", e.currentTarget, e.clientX, e.clientY)
          break
        }
    }
  }

  private _onTouch = (e: TouchEvent) => {
    switch (e.type) {
      case "touchstart":
        {
          window.addEventListener("touchmove", this._onTouch)
          window.addEventListener("touchend", this._onTouch)
          const touch = e.touches[0]
          this._onPointer("down", e.currentTarget, touch.clientX, touch.clientY)
          break
        }
      case "touchend":
        {
          window.removeEventListener("touchmove", this._onTouch)
          window.removeEventListener("touchend", this._onTouch)
          this._onPointer("up", e.currentTarget, 0, 0)
          break
        }
      case "touchmove":
        {
          const touch = e.touches[0]
          this._onPointer("move", e.currentTarget, touch.clientX, touch.clientY)
          break
        }
    }
  }

  private _onPointer = (event: "up" | "move" | "down", target: EventTarget | null, x: number, y: number) => {
    switch (event) {
      case "move":
        {
          if (this.state === "move_hue") {
            this._setHuePosition(
              y - this.rects.hue.top,
            )
            this._updateHueFromPosition()
          } else if (this.state === "move_saturation") {
            this._setSaturationPosition(
              x - this.rects.saturation.left,
              y - this.rects.saturation.top,
            )
            this._updateSaturationFromPosition()
          }
          break
        }
      case "down":
        {
          if (target === this.$saturation) {
            this._updateRects()
            this.state = "move_saturation"
            this._onPointer("move", target, x, y)
          } else if (target === this.$hue) {
            this._updateRects()
            this.state = "move_hue"
            this._onPointer("move", target, x, y)
          }
          break
        }
      case "up":
        {
          this.state = "idle"
          break
        }
    }
  }

  static useDefaultCSS(picker: ColorPicker) {
    picker.$hue.style.cssText = `
    position: relative;
    background: linear-gradient(rgb(255, 0, 0) 0%, rgb(255, 0, 255) 17%, rgb(0, 0, 255) 34%, rgb(0, 255, 255) 50%, rgb(0, 255, 0) 67%, rgb(255, 255, 0) 84%, rgb(255, 0, 0) 100%);
    `
    picker.$hueSelector.style.cssText = `
    position: absolute;
    background: white;
    border-bottom: 1px solid black;
    right: -3px;
    width: 10px;
    height: 2px;
    `
    picker.$saturation.style.cssText = `
    position: relative;
    `
    picker.$saturationSelector.style.cssText = `
    border: 2px solid white;
    position: absolute;
    width: 14px;
    height: 14px;
    background: white;
    border-radius: 10px;
    top: -7px;
    left: -7px;
    box-sizing: border-box;
    z-index 10;
    `
    picker.$brightness.style.cssText = `
    width: 100%;
    height: 100%;
    background: linear-gradient(rgba(255, 255, 255, 0), rgba(0, 0, 0, 1));
    `
  }
}