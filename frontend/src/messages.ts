export * from "@graphite/../wasm/pkg/graphite_wasm";

// Type convert a union of messages into a map of messages
export type ToMessageMap<T> = {
	[K in T extends string ? T : T extends object ? keyof T : never]: K extends T ? Record<string, never> : T extends Record<K, infer Payload> ? Payload : never;
};

import type { FrontendMessage } from "@graphite/../wasm/pkg/graphite_wasm";
export type FrontendMessages = ToMessageMap<FrontendMessage>;
