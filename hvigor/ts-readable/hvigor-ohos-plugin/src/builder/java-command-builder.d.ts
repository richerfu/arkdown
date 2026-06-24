/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/java-command-builder.d.ts
 */
import { BaseCommandBuilder } from './base-command-builder';
export declare class JavaCommandBuilder extends BaseCommandBuilder {
    private readonly _javaSystemList;
    constructor();
    addCalledJarFile(jarFile: string): JavaCommandBuilder;
    addJvmOption(jvmOption: string): JavaCommandBuilder;
    build(): string[];
    /**
     * 声明java的系统变量, 格式为-D[key]=[value]
     *
     * @param {string} key
     * @param {string} value
     * @returns {JavaCommandBuilder}
     */
    setJavaSystemProperty(key: string, value: string): JavaCommandBuilder;
}
