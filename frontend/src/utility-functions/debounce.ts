export type Debouncer = ReturnType<typeof debouncer>;

export type DebouncerOptions = {
	debounceTime: number;
};

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function debouncer<T>(callFn: (value: T) => unknown, { debounceTime = 60 }: Partial<DebouncerOptions> = {}) {
	let currentValue: T | undefined;

	const emitValue = (): void => {
		if (currentValue === undefined) {
			throw new Error("Tried to emit undefined value from debouncer. This should never be possible");
		}
		const emittingValue = currentValue;
		currentValue = undefined;
		callFn(emittingValue);
	};

	const updateValue = (newValue: T): void => {
		if (currentValue !== undefined) {
			currentValue = newValue;
			return;
		}

		currentValue = newValue;
		setTimeout(emitValue, debounceTime);
	};

	return { updateValue };
}
