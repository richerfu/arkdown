/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/util/task-path-util.d.ts
 */
export declare function validateTaskPath(taskPath: string): string[];
/**
 * 通过Task的path分割获取其所属的Module name
 *
 * @param {string} taskPath
 * @returns {string}
 */
export declare function getModuleNameFromTaskPath(taskPath: string): string;
/**
 * 通过Task的path分割获取其所属的Task name
 *
 * @param {string} taskPath
 * @returns {string}
 */
export declare function getTaskNameFromTaskPath(taskPath: string): string;
/**
 * 拼接module name和task name
 *
 * @param {string} moduleName 如果是project则为“”,如果是module则是moduleName
 * @param {string} taskName taskName
 * @returns {string} taskPath
 */
export declare function union(moduleName: string, taskName: string): string;
export declare const parseTaskPath: (taskPath: string) => {
    moduleName: string;
    taskName: string;
};
