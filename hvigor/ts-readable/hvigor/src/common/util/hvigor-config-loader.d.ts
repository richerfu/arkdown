/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/hvigor-config-loader.d.ts
 */
import { HvigorConfig } from '../../base/util/hvigor-config-reader';
import { HvigorCliOptions } from '../../base/util/options/hvigor-cli-options';
export declare class HvigorConfigLoader {
    private readonly hvigorConfig;
    private readonly configPropertyMap;
    private static instance;
    private static config;
    /**
     * 初始化读取两个配置文件 userHome下的和项目目录下的
     *
     */
    constructor();
    static init(cliOpts?: HvigorCliOptions): void;
    private static initCommandLineProperties;
    private static convertToParamValue;
    getHvigorConfig(): HvigorConfig | undefined;
    /**
     * 获取properties中的某个配置，会优先尝试读取命令行中是否存在这个数据，然后是工程下hvigor-config.jons5中
     * 最后是userHome下的hvigor-config.json5
     *
     * @param key 配置名
     * @return 读取到的值
     */
    getPropertiesConfigValue<T>(key: string): T | undefined;
    /**
     *  daemon模式下，常驻线程会保存上一次的静态变量 instance, 如果两次构建中间 hvigor-config.json被修改 instance就需要被更新
     */
    static clean(): void;
    /**
     * 获取线程级存储的config loader单例
     */
    static getInstance(): HvigorConfigLoader;
    /**
     * 尝试在启动的命令行(process.argv)中找到所有的config，不能使用命令行工具commands解析，会和工程中其他的内容冲突
     *
     * @return 所有的config组成的数组
     */
    private static getConfigs;
    private static setConfig;
    /**
     * 解析
     * @param value
     * @private
     */
    private parseConfigValue;
}
