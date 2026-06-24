/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/legacy-project-model-impl.d.ts
 */
import { Project } from '@ohos/hvigor';
import { CoreProjectModelImpl } from './core-project-model-impl';
export declare class LegacyProjectModelImpl extends CoreProjectModelImpl {
    constructor(project: Project);
    initSubModules(): void;
    getDefaultBundleName(): string;
    getBundleType(): string;
    isFaMode(): boolean;
}
