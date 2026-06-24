/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/ohos-test-util.d.ts
 */
export declare function isTemplatesAfterSourceCodeMigration(srcOhosTestPath: string): boolean;
export declare function getOhosTestIntermediatesSrcOhosTestRelativePath(): string;
export declare function getOhosTestIntermediatesEtsPath(modulePath: string): string;
export declare function getOhosTestAbilityConfig(): {
    description: string;
    mainElement: string;
    pages: string;
    abilities: {
        name: string;
        srcEntry: string;
        description: string;
        label: string;
        exported: boolean;
    }[];
};
