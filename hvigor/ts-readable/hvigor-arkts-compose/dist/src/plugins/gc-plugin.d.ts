/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/gc-plugin.d.ts
 */
import { SharedPluginContext } from '@ohos/hvigor-arkts-base';
export declare function gcPlugin(): {
    name: string;
    beforeBuild: typeof gcBeforeBuild;
    transform: typeof gcPluginTransform;
    afterBuildStart: {
        order: string;
        handler(): void;
    };
};
declare function gcBeforeBuild(this: SharedPluginContext): void;
declare function gcPluginTransform(code: string, id: string): Promise<void>;
export {};
