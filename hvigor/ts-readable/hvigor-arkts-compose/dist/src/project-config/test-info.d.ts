/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/test-info.d.ts
 */
export interface TestInfo {
    /**
     * 测试框架
     * testMode 测试模式ohosTest|unitTest
     * coveragePathFilter 覆盖率路径过滤
     * coverageMode 覆盖插桩模式：黑白盒
     */
    testFrameworkPar?: {
        testMode?: string;
        coveragePathFilter?: string;
        coverageMode?: 'black' | 'white';
    };
    /**
     * 是否需要进行代码覆盖率插装
     * true是，false否
     */
    needCoverageInsert?: boolean;
}
