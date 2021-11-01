module.exports = function VueSvgLoader(svg) {
	this.cacheable();
	return `<template>${svg}</template>`;
};
