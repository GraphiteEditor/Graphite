// TODO: Try and get rid of the need for this file

export interface TextButtonWidget {
	tooltip?: string;
	message?: string | object;
	callback?: () => void;
	props: {
		kind: "TextButton";
		label: string;
		emphasized?: boolean;
		minWidth?: number;
		disabled?: boolean;

		// Callbacks
		// `action` is used via `IconButtonWidget.callback`
	};
}
