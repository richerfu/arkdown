/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/plugins/terser-plugin.d.ts
 */
import { NormalizedOutputOptions, RenderedChunk } from 'rollup';
import { MinifyOptions } from 'terser';
export declare function terserPlugin(options?: MinifyOptions): {
    name: string;
    renderChunk(code: string, chunk: RenderedChunk, outputOptions: NormalizedOutputOptions): Promise<{
        code: string;
        map: any;
    }>;
};
