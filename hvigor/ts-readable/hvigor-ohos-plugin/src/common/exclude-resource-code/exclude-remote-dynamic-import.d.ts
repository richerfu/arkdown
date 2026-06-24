/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/exclude-resource-code/exclude-remote-dynamic-import.d.ts
 */
import { Dependency } from "../../project/dependency/core/dependency-interface";
import { RuntimeOnlyObj } from "../../options/build/build-opt";
/**
 * 收集模块级build-profile.json5中的excludePackages配置
 *
 * @param runtimeOnly build-profile.json5中buildOption对象
 * @returns 收集到的excludePackages数组
 */
export declare function collectFromBuildOption(runtimeOnly: RuntimeOnlyObj): string[];
/**
 * 检查给定的依赖项是否需要被排除
 * @param {Set<string>} excludePackages - 需要被排除的包集合
 * @param {Dependency[]} remoteHarDep - 需要检查的远程Har依赖项数组
 * @param {string} depName - 需要检查的依赖项名称
 * @return {boolean} 如果给定的依赖项需要被排除，返回true，否则返回false
 */
export declare function isNeedToExcludeHar(excludePackages: Set<string>, remoteHarDep: Dependency[], depName: string): boolean;
