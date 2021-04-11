import { Emitter, EmitterCallback, EmitterEvent } from "lol/js/emitter"

interface NCEvents {
  "update_primary_color": { color: { r: number, g: number, b: number } }
  "update_secondary_color": { color: { r: number, g: number, b: number } }
}

export type NCEvent<K extends keyof NCEvents = any> = EmitterEvent<K, NCEvents[K]>
export type NCEventCallback<K extends keyof NCEvents = any> = EmitterCallback<K, NCEvents[K]>
export const NC = new Emitter<NCEvents>()