/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/module-build-profile-loader.d.ts
 */
import { ModuleBuildProfile } from '../../../options/build/module-build-profile';
import { AbstractBuildProfileLoader } from './abstract-build-profile-loader';
declare class ModuleBuildProfileLoader extends AbstractBuildProfileLoader {
    getBuildProfile(nodeName: string): ModuleBuildProfile.ModuleBuildOpt;
    setBuildProfileJson5Obj(hvigorNodeName: string, buildProfileJson5Obj: ModuleBuildProfile.ModuleBuildOpt, schemaCheck?: boolean): void;
}
export declare const moduleBuildProfileLoader: ModuleBuildProfileLoader;
export {};
