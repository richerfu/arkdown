/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/add-packagejson-meta-plugin.d.ts
 */
import { MinimalPluginContext } from '@ohos/hvigor-arkts-base';
declare function resolveId(this: MinimalPluginContext, importee: string, importer: string, options: any): {
    meta: {
        packageJson: any;
        isNodeModuleEntryFile: boolean;
    };
    id: any;
} | null;
declare function buildStart(): void;
export declare function addPackageJsonMetaPlugin(): {
    name: string;
    buildStart: typeof buildStart;
    resolveId: typeof resolveId;
};
export {};
