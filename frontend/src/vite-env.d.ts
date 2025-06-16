/// <reference types="node" />
/// <reference types="svelte" />
/// <reference types="svelte/store" />
/// <reference types="svelte/motion" />
/// <reference types="svelte/transition" />
/// <reference types="svelte/animate" />
/// <reference types="svelte/easing" />
/// <reference types="svelte/elements" />
/// <reference types="vite/client" />

/* 
    Why vite-env.d.ts instead of compilerOptions.types inside jsconfig.json or tsconfig.json?

    Setting compilerOptions.types shuts out all other types not explicitly listed in the configuration. 
    Using triple-slash references keeps the default TypeScript setting of accepting type information from the entire workspace, 
    while also adding svelte and vite/client type information.
*/
