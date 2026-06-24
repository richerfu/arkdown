/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pre-build.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { JsonProfile } from '../model/json-profile';
import { ModuleJson } from '../options/configure/module-json-options';
import { OhosLogger } from '../utils/log/ohos-logger';
import { AbstractPreBuild } from './abstract/abstract-pre-build';
import { TargetTaskService } from './service/target-task-service';
/**
 * preBuild Task
 *
 * @since 2022/1/10
 */
export declare class PreBuild extends AbstractPreBuild {
    protected readonly INSIGHT_INTENT_SRC_ENTRY = "insightIntent-srcEntry";
    protected readonly MODULE_SRC_ENTRY = "module-srcEntry";
    protected readonly MODULE_ABILITIES_SRC_ENTRY = "module-abilities-srcEntry";
    protected readonly MODULE_EXTENSION_ABILITIES_SRC_ENTRY = "module-extensionAbilities-srcEntry";
    protected readonly insightIntentJsonPath: string;
    protected readonly targetJsonPath: string;
    protected isPreview: boolean | undefined;
    private readonly appJson5Path;
    private readonly arkDataPath;
    private readonly utdPath;
    private readonly hvigorConfigPath;
    private readonly targetModuleOptObj;
    private readonly resourcePath;
    private readonly pagesJsonPath;
    private readonly systemThemeJsonPath;
    private readonly startWindowJsonPathArr;
    private readonly formJsonPathArr;
    private readonly routerMapJsonPath;
    private readonly shareConfigJsonPath;
    private readonly shortcutsJsonPaths;
    private readonly extendSourceValidateResult;
    private moduleDependencyNames;
    protected get isExpandImportEnabled(): boolean;
    private get excludeExpandImportDependencyNames();
    declareInputs(): Map<string, TaskInputValue>;
    /**
     * 处理widgetSrc路径，并将其添加到输入映射中。
     * @param declareInputsMap - 存储输入值的映射，键为widgetSrc路径，值为TaskInputValue对象。
     */
    private handleWidgetSrc;
    declareInputFiles(): FileSet;
    private setShortcutsJsonPathIncrementalInput;
    /**
     * 设置增量输入
     * @param fileSet 文件集合
     * @param filePath 文件路径
     * @param options 文件系统选项，可选参数
     */
    private setIncrementalInput;
    constructor(taskService: TargetTaskService, taskName?: import("@ohos/hvigor").TaskDetails);
    protected doValidateForDiffApiType(): void;
    protected checkModulesOutsideProjectWithoutModuleInfo(): void;
    /**
     * 判断是否是本地模块信息缺失的异常场景
     * 判断依据：rootPath不包含oh_modules，且不是cpp本地so依赖，理论上是本地模块，但是模块信息缺失了，isLocal会误识别为false，为异常场景
     * @param dependency
     * @param moduleList
     * @private
     */
    private isLocalModuleMissingInfo;
    /**
     * 当存在.preview缓存时移动mock-config.json5配置文件，需要删除.preview中mock-config缓存文件
     * @protected
     */
    protected mockCacheClean(): void;
    protected targetESVerisonValidate(): void;
    protected checkByteCodeHar(): void;
    /**
     * 对compileSdkVersion和compatibleSdkVersion进行校验，对runtimeOS进行校验
     *
     * @protected
     */
    protected buildProfileJsonValidate(): void;
    /**
     * insight_intent.json文件校验
     *
     * @protected
     */
    protected insightIntentJsonValidate(): void;
    protected utdJsonValidate(utdFilePath: string): void;
    protected arkDataJsonValidate(arkDataFilePath: string): void;
    protected appJson5Validate(): void;
    protected validateModuleSrcEntry(logger: OhosLogger, moduleJsonObj: ModuleJson.ModuleOptObj): void;
    /**
     * 验证可执行二进制文件的路径是否有效
     * 检查二进制文件是否存在，并且位于允许的目标目录中（libs/arm64-v8a libs/x86 libs/armeabi-v7a）
     *
     * @param moduleJsonObj 模块JSON配置对象，包含可执行二进制路径信息
     */
    private validateExecutableBinaryPaths;
    /**
     * 检查路径是否在允许的目录中
     */
    private isBinaryPathInAllowedDir;
    protected moduleJson5Validate(): void;
    /**
     * 校验dynamicImport配置中文件配置
     * 仅检查包名是否为本模块oh-package.json5中dependencies的子集;相对路径通过schema校验,且同时检查文件是否存在
     * @private
     */
    protected checkDynamicImportFile(): void;
    /**
     * 检查所有srcEntry字段是否以相对路径的形式配置
     * @private
     */
    protected checkAllSrcEntry(): void;
    /**
     * 校验 srcEntry 列表
     * @param objName 配置项名称
     * @param srcEntryList srcEntry 列表
     * @private
     */
    private validateSrcEntryList;
    /**
     * 校验 extensionAbilities 中配置的 .so 文件的 srcEntry
     * 只针对 extensionAbilities 中配置 .so 进行校验，其他地方配置逻辑不变
     * @param objName 配置项名称
     * @param srcEntry srcEntry 配置值
     * @param errorFilePath 错误文件路径
     */
    private validateExtensionAbilitiesSrcEntryForSo;
    private formJsonValidate;
    private setModuleDependencyNames;
    beforeAlwaysAction(): Promise<void>;
    private checkEntryModuleJson;
    /**
     * 获取指定profile的json文件路径
     * @param profile 指定的profile名称
     * @return 如果无法从传入参数解析出文件名，返回undefined，否则返回profile对应的json文件路径，
     * @throws 如果找不到对应的json文件或者资源目录配置无效，会抛出错误并退出程序
     */
    private getProfileJsonPath;
    /**
     * 获取theme json的路径
     * @return string 返回theme文件的路径
     */
    private getSystemThemeJsonPath;
    /**
     * hap: src/main/resources/base/profile/main_pages.json
     * hsp: src/main/resources/base/profile/main_pages.json
     * har: undefined
     * ohosTest: src/ohosTest/resources/base/profile/test_pages.json
     *
     * @private
     */
    private getPagesJsonPath;
    /**
     * 获取启动窗口的json路径
     * @return {string[]} 返回启动窗口的json路径数组
     */
    private getStartWindowJsonPath;
    private pagesJsonValidate;
    /**
     * startWindowJ.json文件schema校验
     * @private
     */
    private startWindowJsonValidate;
    /**
     * startWindowJ.json文件schema校验
     * @private
     */
    private systemThemeJsonValidate;
    /**
     * routerMap.json文件schema校验
     * @private
     */
    private routerMapJsonValidate;
    /**
     * shareConfig.json文件schema校验
     * @private
     */
    private shareConfigJsonValidate;
    /**
     * crossAppSharedConfig的$profile指向的文件中的uri值不能重复，需要保证uri的唯一性
     * @private
     */
    private checkShareConfigUri;
    /**
     * syscap.json文件schema校验
     * @private
     */
    private sysCapValidate;
    private hvigorConfigValidate;
    /**
     * 1. validate form file is under current module/ formWidgetModule
     * 2. validate form file exists
     *
     * @param formJsonPath string
     */
    private validateFormSrc;
    /**
     * 校验module.json中的mainElement
     * 1、非模块（模块未空），或新版ohosTest模块不进行校验。
     * 2、支持免安装特性的元服务，mainElement不能为空。
     * 3、mainElement的ability需包含icon和label字段。 -- 此规格变更，允许不包含这两个字段。
     * @returns
     */
    private validateMainElement;
    /**
     * 是否是新的ohosTest模板。
     * 说明：ohosTest原来工程模板里有openHarmonyTestRunner.ets这些模板文件，新模板是把testrunner等目录移走了。
     * @private
     */
    private isNewOhosTestTemplate;
    /**
     * AtomicService相关校验
     *
     * @private
     */
    validateAtomicService(): void;
    /**
     * 获取元服务校验报错文件路径
     *
     * @param bundleType  app.json5-bundleType
     * @param installationFree  module.json5-installationFree
     * @private
     */
    private getAtomicServiceErrorPath;
    /**
     * 指定bundleType和installationFree字段校验
     *
     * @param bundleType  app.json5-bundleType
     * @param installationFree  module.json5-installationFree
     * @private
     */
    private validateSpecificField;
    private validateMockConfig;
    protected getJsonProfileByModel(): JsonProfile;
    private hvigorFileConfigValidate;
    /**
     * 检查当前工程的OHMURL解析方式依赖的hsp包是否支持
     *
     * @private
     */
    private checkOHMURL;
    /**
     * 判断OHMUrl是否一致，如果是本地模块默认一致，如果是非本地模块则需要判断一致性，只对hsp模块进行判断
     *
     * @param dependency
     * @param currentOHMUrl
     * @private
     */
    private isEqualOHMUrl;
    private getExtendSourceDirsValidateResult;
    protected checkExtendSourceDirs(): void;
    /**
     * 如果integratedHsp字段hap中配置了则warn告警
     * 如果配置了integratedHsp为true但是useNormalizedOHMUrl为false或者没有配置的话，error报错
     *
     * @private
     */
    private checkIntegratedHspHsp;
    /**
     * 如果generateSharedTgz字段hap中配置了则warn告警
     *
     * @private
     */
    private checkGenerateSharedTgz;
    /**
     *  校验配置的transformLib是否合法
     *
     */
    private checkTransformLib;
    private checkTransformLibFileType;
    /**
     *  检查app.json5 下面configuration 字段的合法性
     * @protected
     */
    protected checkAppConfiguration(): void;
    protected getConfigurationFilePath(configuration: string): string;
    protected addConfigurationToInput(declareInputsMap: Map<string, TaskInputValue>): Map<string, TaskInputValue>;
    /**
     * 校验如"routerMap":"$profile:xxx"等资源引用方式的配置,若对应文件不存在则报错
     * @param configField
     * @private
     */
    private checkResReferenceConfig;
    /**
     * 仅byteCodeHar支持bundleDependencies
     * @private
     */
    private checkBundledDependencies;
    /**
     * 检查当前api版本har/hsp是否支持UIAbility
     * 该功能仅在api≥14支持,一体化IDE中sdk与hvigor配套,若配置compatibleApiVersion小于14则抛出告警
     * @private
     */
    private checkUIAbility;
    private checkSdkVersionMatchLazyImport;
    /**
     * Iterate dynamicDependency list
     * @private
     */
    private iterateDependencies;
    /**
     * Check dynamicDependency types and issue warning for HAR/other types
     * @private
     */
    private checkDynamicDependencyTypes;
    /**
     * 检查在HAP或HAR模块中是否将includeAppScopeRes设置为false
     * 如果有，则需要提示开发者该配置项对HAP和HAR模块不生效
     */
    private checkIncludeAppScopeResInHapOrHar;
    /**
     * 支持构建分布式卡片包的业务校验（括号内有备注说明的不在此函数校验）：
     * （1）卡片包配置规范检查的type字段必须为shared(schema检查)
     * （2）卡片包的formExtensionModule字段值必须为entry或者feature的模块，并且不允许不同的卡片包配置相同关联的hap模块(一一对应)
     * （3）卡片包的module.json5不允许配置任何ability (schema校验)
     * （4）包内卡片和卡片包不支持混用
     * （5）只有entry或者feature类型的模块的module.json才可以配置formWidgetModule（schema校验）,且值必须配置为卡片包模块
     * （6）片禁止使用受限API和组件 卡片包开发态检测能力增强（ets-loader校验）
     * （7）卡片不允许依赖hsp包
     * （8）应用的卡片包和关联的hap包的module.json5的deliveryWithInstall都需要配置为true
     * （9）minSDK的校验，只有6.0的设备才支持卡片包
     * （10）卡片包form_config.json配置联想校验(在formJsonValidate()处校验)
     * @protected
     */
    protected checkFormModule(): void;
    /**
     * 针对hap的校验:
     * 校验4
     *
     * @private
     */
    private checkMixForm;
    /**
     * 针对formExtensionModule的校验:
     * 校验5，9
     *
     * @private
     */
    private checkFormExtensionModule;
    /**
     * formWidgetModule相关校验:
     * 校验2、4、7、8、9
     *
     * @private
     */
    private checkFormWidgetModule;
    private checkNoFormAbility;
    protected checkExpandImportConfigs(): void;
}
