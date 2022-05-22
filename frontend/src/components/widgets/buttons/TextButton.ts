// TODO: Try and get rid of the need for this file

export interface TextButtonWidget {
	kind: "TextButton";
	tooltip?: string;
	message?: string | object;
	callback?: () => void;
	props: {
		// `action` is used via `IconButtonWidget.callback`
		label: string;
		emphasized?: boolean;
		disabled?: boolean;
		minWidth?: number;
		gapAfter?: boolean;
	};
}
