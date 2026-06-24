/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/validate/dependency-module-version-validate.d.ts
 */
export declare function resolveJsonObjWithoutAtModule(ohPackageObj: any, projectDir: string): void;
/**
 * 检测是否配置了@module格式的源码依赖，如果找到了则返回对应的版本号
 *
 * @param originalVersion 原始配置的参数
 * @param projectDir 项目目录
 * @private
 */
export declare function resolveModuleVersion(originalVersion: string, projectDir: string): string;
