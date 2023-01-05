import type { FrontendMessage_keyed } from "./editor_types";
import type { Unsubscribe, Emitter } from "nanoevents";

export type GraphiteEvents = FrontendMessage_keyed;
export type GraphiteCallback<K extends keyof GraphiteEvents> = (arg: GraphiteEvents[K]) => void;

/**
 * I am the event emitter for Graphite. I relay editor events from the WASM module.
 *
 * I am a special Emitter type derived from {@link Emitter}.
 */
export declare class GraphiteEmitter {
	/**
	 * Event names in keys and arrays with listeners in values.
	 *
	 * ```js
	 * emitter1.events = emitter2.events
	 * emitter2.events = { }
	 * ```
	 */
	events: Partial<{ [E in keyof GraphiteEvents]: GraphiteCallback<E>[] }>;

	/**
	 * Add a listener for a given event.
	 *
	 * ```js
	 * const unbind = ee.on('tick', (tickType, tickDuration) => {
	 *   count += 1
	 * })
	 *
	 * disable () {
	 *   unbind()
	 * }
	 * ```
	 *
	 * @param event The event name.
	 * @param cb The listener function.
	 * @returns Unbind listener from event.
	 */
	on<K extends keyof GraphiteEvents>(this: this, event: K, cb: GraphiteCallback<K>): Unsubscribe;

	/**
	 * Calls each of the listeners registered for a given event.
	 *
	 * ```js
	 * ee.emit('tick', tickType, tickDuration)
	 * ```
	 *
	 * @param event The event name.
	 * @param args The arguments for listeners.
	 */
	emit<K extends keyof GraphiteEvents>(this: this, event: K, arg: GraphiteEvents[K]): void;
}
