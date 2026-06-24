/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/resolver/base-package-resolver.d.ts
 */
import { OhosLogger } from '../log/ohos-logger';
export declare class BasePackageResolver {
    private readonly packageFileWord;
    private readonly moduleDirWord;
    constructor(packageFileWord: string, moduleDirWord: string);
    /**
     * 基于类node_modules的结构，向上寻找
     * @param {string} moduleName
     * @param {string} initDir
     * @returns {string | null}
     */
    resolvePackagePath(moduleName: string, initDir: string, log?: OhosLogger): string | undefined;
}
