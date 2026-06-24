/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/pac-util.d.ts
 */
import { PacFile } from './data-structure';
/**
 * 检查pac.json是否包含必选字段, 当前规格为'dataProcess', 'specialAPIs'其中之一
 *
 * @param filePath
 */
export declare function checkPacJsonRequiredFields(filePath: string): boolean;
export declare function checkPacJsonExistAndGetObj(pacPath: string): PacFile | undefined;
export declare function pacFileValidate(moduleName: string, originalPacJsonPath: string, pacSchemaPath: string): boolean;
