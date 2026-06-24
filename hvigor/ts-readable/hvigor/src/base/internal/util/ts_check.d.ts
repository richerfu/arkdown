/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/util/ts_check.d.ts
 */
import ts from 'typescript';
export declare const hvigorfileTSRoots: Set<string>;
export declare const targetRoots: Set<string>;
export declare const targetModules: Set<string>;
export declare const targetModulesRoots: Map<string, string>;
export declare let hvigorConfigFileTsRoot: string | undefined;
/**
 * 以每个hvigorfile.ts和hvigorconfig.ts为入口文件并行进行类型检查
 *
 */
export declare function hvigorfileTypeCheck(): void;
/**
 * compilePlugin 插件语法检查
 *
 * @param compilePluginPath compilePlugin文件绝对路径
 */
export declare function doCompilePluginTypeCheck(compilePluginPath: string): void;
/**
 * 格式化诊断结果
 *
 * @param diagnostic 格式化后前的诊断结果
 * @param host 格式化host
 * @param fileName 指定的诊断结果对应的文件
 * @returns {string} 格式化后的诊断结果
 */
export declare function formatDiagnostic(diagnostic: ts.Diagnostic, host: ts.FormatDiagnosticsHost, fileName?: string): string;
export declare function setHvigorConfigFileTsRoot(root: string): void;
