export interface EmitterListener<K = any, V = any> {
	once: boolean;
	cb: EmitterCallback<K, V>;
}

export interface EmitterEvent<K = string, V = any> {
	event: K;
	value: V;
}

export type EmitterCallback<K = string, V = any> = (event: EmitterEvent<K, V>) => void;

export class Emitter<T = any> {
	private listeners: Record<string, Set<EmitterListener>> = {};

	private getOrCreateListener<K extends keyof T>(name: K) {
		const n = name as string;
		return (this.listeners[n] = this.listeners[n] || new Set<EmitterListener>());
	}

	/**
	 * Listen from native
	 */
	on<K extends keyof T>(name: K, cb: EmitterCallback<K, T[K]>) {
		this.getOrCreateListener(name).add({ once: false, cb });
	}

	/**
	 * Listen from native, once
	 */
	once<K extends keyof T>(name: K, cb: EmitterCallback<K, T[K]>) {
		this.getOrCreateListener(name).add({ once: true, cb });
	}

	/**
	 * Stop listening native event
	 */
	off<K extends keyof T>(name: K, cb: EmitterCallback<K, T[K]>) {
		const listeners = this.getOrCreateListener(name);

		for (const listener of listeners) {
			if (listener.cb == cb) {
				listeners.delete(listener);
				break;
			}
		}
	}

	/**
	 * Called by the native to dispatch an event
	 */
	dispatch<K extends keyof T>(name: K, value?: T[K]) {
		const n = name as string;
		const listeners = this.listeners[n];

		if (listeners && listeners.size > 0) {
			for (const listener of listeners) {
				listener.cb({
					event: name,
					value,
				});
				if (listener.once) listeners.delete(listener);
			}
		}
	}

	clear() {
		this.listeners = {};
	}
}
