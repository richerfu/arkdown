/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/wrapper-chunk-plugin.d.ts
 */
import { OutputOptions, PluginContext, RenderedChunk } from '@ohos/hvigor-arkts-base';
/**
 * 处理chunk的共享变量的导入导出以及添加闭包
 */
export declare function wrapperChunkPlugin(): {
    name: string;
    renderChunk: typeof renderChunk;
};
declare function renderChunk(this: PluginContext, code: string, chunkInfo: RenderedChunk, options: OutputOptions, { chunks }: {
    chunks: Record<string, RenderedChunk>;
}): {
    code: string;
    map: {
        mappings: string;
        version: number;
        names: never[];
        file: null;
        sourcesContent: null[];
        sources: null[];
        toString(): string;
        toUrl(): string;
    } | null;
};
export {};
