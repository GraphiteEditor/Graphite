/* eslint-disable no-useless-escape */
/* eslint-disable quotes */

export function escapeJSON(str: string): string {
	return str
		.replace(/[\\]/g, "\\\\")
		.replace(/[\"]/g, '\\"')
		.replace(/[\/]/g, "\\/")
		.replace(/[\b]/g, "\\b")
		.replace(/[\f]/g, "\\f")
		.replace(/[\n]/g, "\\n")
		.replace(/[\r]/g, "\\r")
		.replace(/[\t]/g, "\\t");
}
