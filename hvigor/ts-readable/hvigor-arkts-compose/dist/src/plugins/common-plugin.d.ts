/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/common-plugin.d.ts
 */
import { CacheStoreManager, MinimalPluginContext } from '@ohos/hvigor-arkts-base';
import { ProjectConfig } from '../project-config/project-config';
export interface ChangedDeps {
    name: string;
    lastRootPath: string | undefined;
}
export declare function commonPlugin(projectConfig: ProjectConfig, cacheStoreManager?: CacheStoreManager): {
    name: string;
    options(this: MinimalPluginContext): void;
    beforeBuild(this: any): void;
    beforeBuildEnd: {
        order: string;
        handler(): Promise<void>;
    };
    transform(this: MinimalPluginContext, _: string, id: string): void;
};
