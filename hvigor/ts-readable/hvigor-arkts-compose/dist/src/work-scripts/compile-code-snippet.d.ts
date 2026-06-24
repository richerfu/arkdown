/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/work-scripts/compile-code-snippet.d.ts
 */
/// <reference types="node" />
import { MessagePort } from 'worker_threads';
export declare function compileCodeSnippet(codeSnippet: object): boolean;
export declare function initServer(cachePath: string | undefined, buildPath: string | undefined, etsLoaderPath: string, parentPort: MessagePort): void;
