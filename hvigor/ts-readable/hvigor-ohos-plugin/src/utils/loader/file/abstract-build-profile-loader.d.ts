/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/abstract-build-profile-loader.d.ts
 */
import { OhosLogger } from '../../log/ohos-logger';
export declare abstract class AbstractBuildProfileLoader {
    protected readonly _log: OhosLogger;
    abstract getBuildProfile(nodeName?: string): any;
    protected getBuildProfileJson5Obj(hvigorNodeName: string, schemaCheck?: boolean, classKind?: string): any;
    abstract setBuildProfileJson5Obj(hvigorNodeName: string, buildProfileJson5Obj: any, schemaCheck: boolean): void;
    protected setBuildProfileJson5ObjCheck(hvigorNodeName: string, buildProfileJson5Obj: any, schemaPath: string | undefined, classKind?: string): void;
    protected validateBuildProfile(obj: object, filePath: string, schemaPath: string): void;
    protected initializeReplace(json5Obj: any): any;
}
