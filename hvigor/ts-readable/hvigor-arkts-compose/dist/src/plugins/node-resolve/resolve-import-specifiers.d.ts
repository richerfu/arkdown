/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/resolve-import-specifiers.d.ts
 */
export declare function resolveImportSpecifiers({ baseDir, exportConditions, extensions, ignoreSideEffectsForRoot, importer, importSpecifierList, mainFields, moduleDirectories, modulePaths, packageInfoCache, preserveSymlinks, rootDir, useBrowserOverrides, warn, caseSensitiveCheck, sdkModulesPath, }: any): Promise<{
    location: string | undefined;
    hasModuleSideEffects: () => null;
    hasPackageEntry: boolean;
    packageBrowserField: boolean;
    packageInfo: undefined;
} | {
    resolvedSpecifierIndex: number;
    location: any;
    hasModuleSideEffects: () => null;
    hasPackageEntry: boolean;
    packageBrowserField: boolean;
    packageInfo: undefined;
} | null>;
