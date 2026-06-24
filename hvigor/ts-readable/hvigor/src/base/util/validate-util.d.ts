/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/validate-util.d.ts
 */
import { ModuleOpt } from '../common/options/project-structure-opt';
/**
 * 工程初始化时校验build-profile.json5
 */
export declare class ValidateUtil {
    private static logger;
    static validateModule(module: ModuleOpt): void;
}
