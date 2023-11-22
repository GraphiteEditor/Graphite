export type Debouncer = ReturnType<typeof debouncer>;

export type DebouncerOptions = {
	debounceTime: number;
};

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function debouncer<T>(callFn: (value: T) => unknown, { debounceTime = 60 }: Partial<DebouncerOptions> = {}) {
	let currentValue: T | undefined;
	let recentlyUpdated: boolean = false;

	const debounceEmitValue = () => {
		recentlyUpdated = false;
		if (currentValue === undefined) return;
		debounceUpdateValue(currentValue);
	};

	const debounceUpdateValue = (newValue: T) => {
		if (recentlyUpdated) {
			currentValue = newValue;
			return;
		}

		callFn(newValue);
		recentlyUpdated = true;
		currentValue = undefined;
		setTimeout(debounceEmitValue, debounceTime);
	};

	return { debounceUpdateValue };
}
