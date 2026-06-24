/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/log/default-configuration.d.ts
 */
import { Configuration, Level } from 'log4js';
export declare const logFilePath: () => string;
export declare const setConfiguration: (config: Configuration) => void;
export declare const getConfiguration: () => Configuration;
export declare const updateConfiguration: () => Configuration;
export declare const setCategoriesLevel: (level: Level, ignoreLevelCategoryFilterArr?: string[]) => void;
export declare const getLevel: () => Level;
export declare const levelMap: Map<string, Level>;
