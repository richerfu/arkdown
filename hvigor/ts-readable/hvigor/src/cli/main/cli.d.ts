/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/main/cli.d.ts
 */
import { HvigorCliOptions } from '../../base/util/options/hvigor-cli-options';
/**
 * 解析hvigor命令行参数
 *
 * @return {HvigorCliOptions} 解析后的命令行参数
 */
export declare function parseCommand(): HvigorCliOptions;
/**
 * hvigor执行入口，根据命令行参数是否启用服务化调用不同的启动接口。
 *
 * @param {HvigorCliOptions} cliArgv hvigor命令行参数
 */
export declare function startHvigorBuild(cliArgv: HvigorCliOptions): Promise<void>;
