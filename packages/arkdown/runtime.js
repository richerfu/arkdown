'use strict';const fs=require("node:fs");const crypto=require("node:crypto");
const Module=require("node:module");const nativeModuleLoad=Module._load;
const os=require("node:os");const path=require("node:path");
const util=require("node:util");const nativeRuntimeBinding=require("./index.js");
const HVIGOR_PARITY_VERSION=`6.24.3`;const OHOS_COMMAND_BUILDER_TYPE=Object.freeze({
0:`NONE`,1:`CMAKE`,2:`NINJA`,NONE:0,CMAKE:1,NINJA:2});function createHvigorPublicUtilities(host,options={
}){const official=nativeModuleLoad.call(Module,`@ohos/hvigor`,module,false);
  const exports={...official};exports.HvigorSystemPlugin=class HvigorSystemPlugin extends official.HvigorSystemPlugin{
  constructor(pluginId){super(pluginId);this.pluginVersion=HVIGOR_PARITY_VERSION}
    };exports.NormalizedFile=nativeModuleLoad.call(Module,`@ohos/hvigor/src/common/util/hvigor-file-util.js`,module,false).NormalizedFile;
  exports.TaskPendingPromises=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/internal/task/core/task-pending-promises.js`,
  module,false).TaskPendingPromises;exports.ProjectImpl=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/internal/lifecycle/hvigor-node-impl/project-impl.js`,
  module,false).ProjectImpl;exports.ModuleImpl=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/internal/lifecycle/hvigor-node-impl/module-impl.js`,
  module,false).ModuleImpl;exports.HvigorNodeImpl=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/vigor/plugin/impl/hvigor-node-impl.js`,
  module,false).HvigorNodeImpl;exports.HvigorLifecycleHook=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/internal/lifecycle/hook/hvigor-lifecycle-hook.js`,
  module,false).HvigorLifecycleHook;host._officialConfiguration=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/internal/lifecycle/configuration.js`,
  module,false).configuration;const pathConstants=nativeModuleLoad.call(Module,`@ohos/hvigor/src/common/const/path-const.js`,
  module,false);pathConstants.HVIGOR_PROJECT_ROOT_DIR=host.projectRoot;
  pathConstants.HVIGOR_PROJECT_WRAPPER_HOME=path.resolve(host.projectRoot,`hvigor`);
  pathConstants.HVIGOR_PROJECT_CACHE_PATH=host.hvigorCacheDir();
  pathConstants.HVIGOR_USER_HOME=getHvigorUserHome();official.coreParameter.clean();
  official.coreParameter.properties=host._coreParameter.properties;
  official.coreParameter.extParams=host._coreParameter.extParams;
  Object.assign(official.coreParameter.startParams,host._coreParameter.startParams);
  official.coreParameter.startParams.logLevel=official.levelMap.get(String(official.coreParameter.startParams.logLevel||`info`).toLowerCase())||official.levelMap.get(`info`);
  official.coreParameter.workspaceDir=host._coreParameter.workspaceDir;
  host._coreParameter=official.coreParameter;host._parameter=official.hvigorCore.getParameter();
  class PathUtil extends official.PathUtil{static getHvigorCacheDir(logger){
    return host.hvigorCacheDir(logger)}static getReportDirPath(){
    return path.resolve(this.getHvigorCacheDir(),`report`)}}
  const configuration=official.getConfiguration();const configureHvigorLogging=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/log/hvigor-log.js`,
  module,false).configure;configuration.appenders[`debug-log-file`].filename=path.resolve(options.cacheDir||host.hvigorCacheDir(),
  `outputs`,`build-logs`,`build.log`);let consoleOwner=options.consoleOwner===`tracing`?`tracing`:`log4js`;
  const nativeAppender={configure:()=>event=>{const level=event.level.levelStr.toLowerCase();
      if(!nativeRuntimeBinding.runtimeObservabilityIsEnabled(level,`arkdown_napi::hvigor`))return;
      nativeRuntimeBinding.emitRuntimeLog({level,category:event.categoryName,message:official.replaceBundleName(util.format(...event.data),
        host._appJson?.app?.bundleName||host._projectProfile?.app?.bundleName),source:`js_plugin`}
      )}};const activeLoggingConfiguration=()=>{const active={
    ...configuration,appenders:Object.fromEntries(Object.entries(configuration.appenders).map(([name,value])=>[name,{
      ...value}])),categories:Object.fromEntries(Object.entries(configuration.categories).map(([name,value])=>[name,{
      ...value,appenders:[...value.appenders]}]))};delete active.appenders[`debug-log-file`];
    delete active.categories[`debug-file`];active.appenders[`arkdown-native`]={
    type:nativeAppender};active.appenders.silent||={type:`stdout`,layout:{
      type:`dummy`}};active.categories[`debug-file`]={appenders:[`silent`],level:`off`}
    ;for(const value of Object.values(active.categories)){if(value.level!==`off`&&!value.appenders.includes(`arkdown-native`))value.appenders.push(`arkdown-native`)}
    if(consoleOwner===`tracing`){active.categories.default.appenders=[`arkdown-native`];
      active.categories[`no-pattern-info`].appenders=[`arkdown-native`]}
    return active};const applyConsoleOwner=owner=>{consoleOwner=owner===`tracing`?`tracing`:`log4js`;
    configureHvigorLogging(activeLoggingConfiguration());host._observabilityEnabledCache=new Map}
  ;const configuredLogLevel=String(options.logLevel??`debug`).toLowerCase();
  const logLevel=official.levelMap.get(configuredLogLevel)||official.levelMap.get(`debug`);
  official.setCategoriesLevel(logLevel);for(const[category,value]of Object.entries(configuration.categories)){
  if(!category.includes(`file`))value.level=logLevel.levelStr.toLowerCase()}
  applyConsoleOwner(consoleOwner);official.HvigorConfigLoader.clean();
  class TraceManager extends official.TraceManager{flush(){for(const callback of Object.values(this.callBackList))callback();
      if(!host.observabilityInstalled())host.installObservability();
      return nativeRuntimeBinding.persistRuntimeDetails(JSON.stringify(structuredClone(this.data),official.replacer))}
    static trace(key,data,callback){this.instance||=new TraceManager;
      this.instance.trace(key,data,callback)}}class ReportServiceImpl extends official.ReportServiceImpl{
  constructor(){super();this.reportListeners=[official.MetricService.getInstance()]}
    static getInstance(){this.instance||=new ReportServiceImpl;
      return this.instance}report(){if(!host.observabilityInstalled())host.installObservability();
      ReportServiceImpl.buildId=exports.globalData.buildId;return nativeRuntimeBinding.persistRuntimeReport(JSON.stringify(this.getReport(),
      official.replacer),official.coreParameter.properties[`hvigor.analyzeHtml`]===true)}
    startProcessMonitor(){this.processMemorySampler||=()=>process.memoryUsage();
      this.processMonitor||=new nativeRuntimeBinding.RuntimeProcessMonitor(this.processMemorySampler,process.pid);
      this.processMonitor.start()}stopProcessMonitor(){this.processMonitor?.stop()}
    }const{wrapIncrementalDeclarations}=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/incremental-decorator/wrap.js`,
  module,false);applyConsoleOwner(consoleOwner);const bindTaskToSession=(task,sync=false)=>{
  wrapIncrementalDeclarations(task);task.__arkdownClassTask=true;
    task.__arkdownSyncTask=sync;task.taskLog=official.HvigorLogger.getLogger(task.getPath());
    return task};const addExtensionHandler=suffixes=>{for(const suffix of suffixes){
    Module._extensions[suffix]=(mod,filename)=>{mod._compile(compileHvigorScriptSync(path.resolve(filename),Boolean(host.startParameters().printStackTrace)),
        filename)}}};exports.PathUtil=PathUtil;exports.bindTaskToSession=bindTaskToSession;
  exports.TraceManager=TraceManager;exports.ReportServiceImpl=ReportServiceImpl;
  exports.startEnvironment={nodeHome:host.config.nodeHome??process.env.NODE_HOME??``,workspaceDir:host._parameter.getWorkspaceDirInternal()}
  ;official.globalData.init({...process.env,cwd:host.projectRoot,configProps:new Map([[`prop`,new Map(Object.entries(host._parameter.getExtParams()))]])}
  ,{...host.config,_:host._commandEntryTasks});exports.globalData=official.globalData;
  exports.doCompilePluginTypeCheck=pluginFile=>host._typeCheckCompilePlugin(pluginFile);
  exports.addExtensionHandler=addExtensionHandler;exports.applyConsoleOwner=applyConsoleOwner;
  return exports}const PLUGIN_DISPATCH_HOOKS=[`buildStart`,`resolveId`,`load`,`transform`,`moduleParsed`,`resolveDynamicImport`,
`beforeBuildEnd`,`buildEnd`,`shouldInvalidCache`];const ALL_PLUGIN_HOOKS=new Set(PLUGIN_DISPATCH_HOOKS);
let hvigorTypeScriptExtensionInstalled=false;function createBuild(nativeBuild){
return(config={},plugins)=>new HvigorCommandInvocation(nativeBuild,config,plugins,{
  },false).run()}function createHvigorCommand(nativeBuild,nativeRuntime={
}){return(config={},plugins)=>new HvigorCommandInvocation(nativeBuild,config,plugins,nativeRuntime).run()}
class HvigorCommandInvocation{constructor(nativeBuild,config,plugins,nativeRuntime={
  },commandMode=true){this.nativeBuild=nativeBuild;this.nativeRuntime=nativeRuntime;
    this.runtime=new RuntimePluginLoader(config);this.explicitPlugins=plugins;
    this.commandMode=commandMode}async run(){const startedAt=Date.now();
    let report;let result;let failure=null;const host=this.runtime.hvigorHost;
    const invocationToken=nativeRuntimeBinding.beginRuntimeInvocation(this.runtime.projectRoot);
    try{host.installObservability();const loadedPlugins=await this.runtime.load();
      const plugins=[...loadedPlugins,...normalizePluginList(this.explicitPlugins,this.runtime.projectRoot)];
      await host.runTaskGraphResolved();host.typeCheckBuildScripts();
      if(this.commandMode){report=await this.execute(host.commandPlan(),plugins);
        result={artifacts:report.artifacts,elapsedMs:Date.now()-startedAt,output:report.output}
        ;if(this.runtime.config.sync)await host.writeSyncInfo()}
      else{host.prepareDirectBuildCycle();try{result=await this.nativeBuild(this.runtime.toNativeBuildConfig(),plugins,host.createNativeTaskBridge())}
        finally{host.clearNativeTaskSelection()}}await host.writeBuildMeta();
      return result}catch(error){failure=error;throw error}finally{
    try{if(host.observabilityInstalled())await host.runBuildFinished(failure,result||report);
        else await host._workerPool?.terminate()}finally{try{
        host.finishObservability()}finally{nativeRuntimeBinding.finishRuntimeInvocation(invocationToken)}
        }}}async execute(plan,plugins){const host=this.runtime.hvigorHost;
    const taskEntries=new Map(plan.tasks.map(entry=>[entry.taskPath,entry]));
    const materializedTasks=new Map;const materialize=taskPath=>{
    if(!materializedTasks.has(taskPath))materializedTasks.set(taskPath,host._commandTaskFromPath(taskPath));
      return materializedTasks.get(taskPath)};const customRoots=plan.tasks.filter(entry=>entry.executionLane===`custom`).map(entry=>materialize(entry.taskPath));
    return nativeRuntimeBinding.executeRuntimeCommandPlan(plan,{
    executeStandalone:taskPath=>host.executeCommandTask({...taskEntries.get(taskPath),task:materialize(taskPath)}
      ,this.nativeRuntime.runRuntimeTasks),executeCustom:taskPaths=>host.executeCustomCommandTasks(taskPaths.map(materialize),
      this.nativeRuntime.runRuntimeTasks),executeNative:async request=>{
      const nativeConfig=this.runtime.toNativeBuildConfig();
        nativeConfig.target=request.outputTarget??request.target;
        nativeConfig.module=request.module;nativeConfig.buildTargetName=request.targetName;
        if(request.cutoffTaskRoots.length>0){nativeConfig.nativeTaskRootsJson=JSON.stringify(request.cutoffTaskRoots)}
        nativeConfig.watch=false;host.prepareNativeCommandCycle(request.nodeNames,customRoots,request.rootTaskPaths.map(materialize));
        let result;try{result=await this.nativeBuild(nativeConfig,plugins,host.createNativeTaskBridge())}
        finally{host.clearNativeTaskSelection()}return toArray(result?.artifacts)}
      })}}class RuntimePluginLoader{constructor(config){this.config=config||{
    };this.projectRoot=path.resolve(this.config.project||process.cwd());
    this.hvigorHost=new HvigorRuntimeHost(this.projectRoot,this.config);
    this.projectRoot=this.hvigorHost.projectRoot;this.hvigorWorkspaceNodeModules=null;
    this.hvigorWorkspacePrepared=false;this.seen=new Set}async load(){
  await this.prepareHvigorDependencyWorkspace();await this.hvigorHost.evaluateHvigorConfig();
    const plugins=[...await this.loadConfiguredPluginPaths(),...await this.loadHvigorfiles()];
    await this.hvigorHost.prepareOhpmDependencyWorkspace();plugins.push(...await this.loadBuildProfileCompilePlugins());
    return new HvigorCompilePluginPlanner(this.config).apply(plugins)}
  async prepareHvigorDependencyWorkspace(){if(this.hvigorWorkspacePrepared)return;
    const response=await this.hvigorHost._runtimeSession.prepareHvigorPluginWorkspace(os.homedir(),process.execPath,process.version);
    if(response===null){this.hvigorWorkspacePrepared=true;return}
    if(response.error){const{code,message,...details}=response.error;
      throw this.hvigorHost._error(code,message,details)}this.hvigorWorkspaceNodeModules=configureHvigorNodePath(this.projectRoot,
    response.workspace,response.nodeModules);this.hvigorHost.hvigorWorkspaceNodeModules=this.hvigorWorkspaceNodeModules;
    this.hvigorWorkspacePrepared=true}toNativeBuildConfig(){
  const startParams=this.hvigorHost.startParameters();const outcome=this.hvigorHost._runtimeSession.prepareNativeBuildConfig(this.config,
    {properties:this.hvigorHost._parameter.getPropertiesInternal(),extParams:this.hvigorHost._parameter.getExtParamsInternal(),
      coreStartParams:startParams,defaultCache:Boolean(startParams.incrementalExecution),mode:this.hvigorHost._effectiveBuildMode(),
      fastBuildApp:this.hvigorHost.fastBuildApp()},process.execPath);
    this.hvigorHost._throwBuildScriptDiagnostic(outcome.diagnostic);
    if(!outcome.config){throw this.hvigorHost._error(`ARKDOWN_NATIVE_BUILD_CONFIG_MISSING`,`Rust build-script runtime session returned no native build config`)}
    return outcome.config}async loadConfiguredPluginPaths(){
  const plugins=[];for(const key of[`hvigorPluginFile`,`compilePluginPath`]){
    plugins.push(...await this.loadPluginCandidates(this.config[key],this.projectRoot,{
      requireDefault:true}))}return plugins}async loadHvigorfiles(){
  const plugins=[];const files=this.hvigorHost.discoverBuildScriptFiles(this.discoverHvigorRoots());
    const records=await this.hvigorHost.configureHvigorfiles(files);
    for(const record of records){for(const plugin of await this.collectHvigorfileEffects(record)){
      plugins.push(plugin)}}return plugins}discoverHvigorRoots(){
  return[this.projectRoot,...this.hvigorHost.moduleRoots()]}
  async collectHvigorfileEffects({file,value}){const host=this.hvigorHost;
    const plugins=[];const baseDir=path.dirname(file);const moduleRoot=path.resolve(baseDir)===this.projectRoot?undefined:path.resolve(baseDir);
    const effects=host.effectsFor(file);const capturedBuildOption=this.captureHvigorOverrides(baseDir,value,effects,file);
    for(const plugin of normalizePluginList(value,baseDir)){
    plugins.push(plugin)}const exportedPluginPaths=this.pluginPathPlan(value,baseDir);
    const loadOptions={requireDefault:true,moduleRoot};plugins.push(...await this.loadPluginCandidates(exportedPluginPaths.hvigorPluginFiles,
    baseDir,loadOptions));plugins.push(...await this.loadPluginCandidates(exportedPluginPaths.compilePluginFiles,baseDir,
    loadOptions));const capturedPluginPaths=this.pluginPathPlan(capturedBuildOption,baseDir,false);
    plugins.push(...await this.loadPluginCandidates(capturedPluginPaths.compilePluginFiles,baseDir,loadOptions));
    plugins.push(...await this.loadPluginCandidates(effects.hvigorPluginFile,baseDir,loadOptions));
    plugins.push(...await this.loadPluginCandidates(effects.compilePluginPath,baseDir,loadOptions));
    return plugins}async loadBuildProfileCompilePlugins(){const plugins=[];
    for(const record of this.hvigorHost.compilePluginRecords()){
    plugins.push(...await this.loadPluginCandidates(record.file,record.moduleRoot,{
      requireDefault:true,moduleRoot:record.moduleRoot,targetName:record.targetName}
      ))}return plugins}pluginPathPlan(value,baseDir,includeCompilePluginFile=true){
  return nativeRuntimeBinding.collectBuildScriptPluginPaths(value,baseDir,includeCompilePluginFile)}
  captureHvigorOverrides(root,value,effects,file=path.join(root,`hvigorfile.ts`)){
  const host=this.hvigorHost;const ohos=value&&typeof value===`object`?value.config?.ohos:undefined;
    const overrides=ohos&&typeof ohos===`object`?ohos.overrides:undefined;
    const normalizedRoot=path.resolve(root);const response=host._runtimeSession.captureHvigorOverrides(normalizedRoot,overrides,
    effects.buildOption,file);host._throwBuildScriptDiagnostic(response.diagnostic);
    return response.buildOption}async loadPluginPath(candidate,baseDir=this.projectRoot,options={
  }){if(!candidate||typeof candidate!==`string`)return[];const file=nativeRuntimeBinding.resolveBuildScriptPluginPath(candidate,
    baseDir);const moduleRoot=options.moduleRoot?path.resolve(options.moduleRoot):undefined;
    const targetName=typeof options.targetName===`string`?options.targetName:undefined;
    const identity=`${file||``}\0${moduleRoot||`*`}\0${targetName||`*`}`;
    if(!file||this.seen.has(identity)){return[]}this.seen.add(identity);
    const exports=await loadModule(file);const value=options.requireDefault?requireDefaultExport(exports,file):unwrapDefault(exports);
    return normalizeSortedPluginList(value,path.dirname(file),file,moduleRoot,targetName)}
  async loadPluginCandidates(candidates,baseDir,options){const plugins=[];
    for(const candidate of toArray(candidates))plugins.push(...await this.loadPluginPath(candidate,baseDir,options));
    return plugins}}class HvigorTaskCatalog{constructor(host,node){
  this.host=host;this.node=node;this.taskDefinitions=undefined}
  invalidate(){this.taskDefinitions=undefined}get(name){if(!this.taskDefinitions)this.taskDefinitions=this._load();
    return this.taskDefinitions.get(name)}has(name){return this.get(name)!==undefined}
  entries(){if(!this.taskDefinitions)this.taskDefinitions=this._load();
    return this.taskDefinitions.entries()}request(){const catalog=this.host._runtimePlan().taskCatalogs.find(candidate=>candidate.projectTask===this.node.isProject&&candidate.nodeName===this.node.name);
    if(catalog)return catalog;throw this.host._error(`ARKDOWN_RUNTIME_TASK_CATALOG_NOT_FOUND`,`Rust runtime plan does not contain a task catalog for ${this.node.name}`)}
  _load(){const request=this.request();const entries=nativeRuntimeBinding.runtimeTaskCatalog(request);
    return new Map(entries.map(entry=>[entry.name,entry]))}}
class HvigorRuntimeHost{constructor(projectRoot=process.cwd(),config={
  }){this.projectRoot=path.resolve(projectRoot);this.config=config||{
    };this._nodesByFile=new Map;this._nodeAliasesByFile=new Map;
    this._nodesByName=new Map;this._evaluatedBuildScriptValues=new Map;
    this._fallbackEffects=this._newEffects();this._effects=this._fallbackEffects;
    this._activeFile=null;this.hvigorWorkspaceNodeModules=null;
    this._commandEntryTasks=toArray(this.config.tasks??this.config.task);
    try{this._runtimeSession=new nativeRuntimeBinding.RuntimeBuildScriptSession(this.projectRoot,getHvigorUserHome(),this.config,
      this._commandEntryTasks)}catch(cause){const marker=`ARKDOWN_BUILD_SCRIPT_RUNTIME_DIAGNOSTIC:`;
      const markerIndex=cause.message.indexOf(marker);if(markerIndex>=0){
      const diagnostic=JSON.parse(cause.message.slice(markerIndex+marker.length));
        const{code,message,...details}=diagnostic;throw this._error(code,message,details)}
      throw this._error(`INVALID_HVIGOR_MODEL_FILE`,`failed to load the Rust build-script runtime session for ${this.projectRoot}: ${cause.message}`,
      {cause})}const runtimeModel=this._runtimeSession.projectModel();
    this.projectRoot=path.resolve(runtimeModel.projectRoot);
    this._projectProfile=runtimeModel.projectProfile||{};this._appJson=runtimeModel.appJson;
    this._buildConfigFile=runtimeModel.buildConfigFile;this._fallbackModuleRoots=toArray(runtimeModel.fallbackModuleRoots).map(root=>path.resolve(root));
    this._selectedProductName=this.config.product||this._projectProfile.app?.products?.[0]?.name||`default`;
    this._usesOhpm=this._runtimeSession.usesOhpm;this._projectPackage=this._usesOhpm?runtimeModel.projectPackage||{
    }:runtimeModel.projectNpmPackage||{};this._moduleDescriptors=this._runtimeSession.moduleDescriptors();
    this._parameter=this._makeParameter();this._hvigorConfig=undefined;
    this._hvigorCore=undefined;this._hvigorApi=undefined;this._hvigorExports=new Map;
    this._runtimeBuiltinActions=new Map;this._runtimeTaskFailures=new Map;
    this._hvigorTypeScriptRoots=new Set;this._ohpmPackageModelUpdated=false;
    this._toolingModelRegistry=undefined;this._lifecycle=undefined;
    this._commandBuilderModifiers=new Map;this._syncNodeNames=undefined;
    this._syncVersionChanged=this._readSyncVersionChanged()}
  startParameters(){return this._coreParameter.startParams}_ensurePublicUtilities(){
  if(!this._publicUtilities){this._publicUtilities=createHvigorPublicUtilities(this,this._publicUtilityOptions);
      this._hvigorConfig=this._makeHvigorConfig();this._workerPool=new this._publicUtilities.WorkerPoolDelegator;
      this._toolingModelRegistry=this._publicUtilities.defaultModelRegistry;
      this._toolingModelRegistry.clear();this._lifecycle=this._publicUtilities.HvigorLifecycleHook.getInstance();
      this._lifecycle.clear();this._hvigorCore=this._publicUtilities.hvigorCore;
      this._hvigorCore.setCommandEntryTasks(this._commandEntryTasks);
      this._hvigorCore.setExtraConfig(new Map(Object.entries(this._parameter.getExtParams())))}
    return this._publicUtilities}installObservability(){const consoleToken=String(process.env.ARKDOWN_LOG_CONSOLE||``).trim().toLowerCase();
    const consoleOwner=[`1`,`true`,`yes`].includes(consoleToken)?`tracing`:`log4js`;
    const logLevel=String(this.startParameters().logLevel?.levelStr||this.startParameters().logLevel||`info`).toLowerCase();
    const cacheDir=this.hvigorCacheDir();this._publicUtilityOptions={
    consoleOwner,logLevel,cacheDir};if(this._publicUtilities){
    this._publicUtilities.applyConsoleOwner(consoleOwner)}this._observabilityBuildId||=crypto.randomUUID();
    nativeRuntimeBinding.installRuntimeObservability({mode:`napi`,cacheDir,buildId:this._observabilityBuildId,logLevel,consoleOwner}
    );if(this.startParameters().analyze===1){this._ensurePublicUtilities().ReportServiceImpl.getInstance().startProcessMonitor()}
    this._observabilityInstalled=true}observabilityInstalled(){
  return this._observabilityInstalled===true}finishObservability(){
  if(!this._observabilityInstalled)return;try{nativeRuntimeBinding.finishRuntimeObservability()}
    finally{this._observabilityInstalled=false;this._observabilityEnabledCache=new Map}
    }fastBuildApp(){return Boolean(this._runtimePlan().taskCatalogs.find(catalog=>catalog.projectTask)?.fastBuildApp)}
  _compileApiVersion(){return this._runtimeSession.effectiveCompileApiVersion()}
  _isOhpmProject(){return this._usesOhpm}runtimeProperties(startParams){
  return this.runtimePropertyPlan(startParams).runtimeProperties}
  hvigorCacheDir(logger){const plan=this._runtimePathPlan();
    if(plan.hvigorCacheWarning&&logger)logger.warn(plan.hvigorCacheWarning);
    return plan.hvigorCacheDir}async prepareOhpmDependencyWorkspace(){
  const response=await this._runtimeSession.prepareOhpmDependencyWorkspace({
    properties:this._parameter.getPropertiesInternal(),extParams:this._parameter.getExtParamsInternal(),projectPackage:this._projectPackage,
      modules:this._moduleDescriptors.map(descriptor=>{const node=this._nodesByName.get(descriptor.name);
        return{name:descriptor.name,root:descriptor.root,package:node?.modulePackage??descriptor.modulePackage}
        }),packageModelUpdated:this._ohpmPackageModelUpdated,executable:process.env.ohpmBin}
    );if(response.warning)this._warn(response.warning.code,response.warning.message);
    this._throwBuildScriptDiagnostic(response.error)}async writeSyncInfo(){
  this._ensurePublicUtilities();this._runtimePlan();const customModels={
    };const customModelTaskInfo={};for(const[modelId,bean]of this._toolingModelRegistry.getModelMap()){
    if(typeof modelId!==`string`||typeof bean?.buildModel!==`function`){
      throw this._error(`INVALID_HVIGOR_TOOLING_MODEL`,`ToolingModelBean must expose a string modelId and buildModel() method`)}
      customModels[modelId]=await bean.buildModel();customModelTaskInfo[modelId]=typeof bean.taskInfo===`function`?await bean.taskInfo():[]}
    let projectCustomTasks=[];const moduleCustomTasks={};for(const node of this._nodesByFile.values()){
    const tasks=this._syncCustomTasks(node);if(node.isProject)projectCustomTasks=tasks;
      else moduleCustomTasks[node.name]=tasks}await this._runtimeSession.writeHvigorSyncModel({
    configBuildVersion:this.config.buildVersion,selectedNodeNames:this._syncNodeNames?Array.from(this._syncNodeNames):undefined,
      projectCustomTasks,moduleCustomTasks,customModels,customModelTaskInfo}
    ,this._parameter.getExtParamsInternal())}async writeBuildMeta(){
  await this._runtimeSession.writeHvigorBuildMeta(this._parameter.getPropertiesInternal(),this._parameter.getExtParamsInternal());
    this._syncVersionChanged=false}_readSyncVersionChanged(){
  return this._runtimeSession.hvigorSyncVersionChanged(this._parameter.getPropertiesInternal(),this._parameter.getExtParamsInternal())}
  _syncCustomTasks(node){const realizedNames=new Set;const tasks=node.coreWrapper.getAllTasks().filter(task=>{
    realizedNames.add(task.getName());return!task.__arkdownBuiltInDefinition}
    ).map(task=>({taskName:task.getName(),group:task.getGroup?.()||`Customize`,description:task.getDescription?.()||``,sourcePluginId:task.getSourcePluginId?.()||``}
    ));for(const creation of node.coreWrapper.getAllTasksCA()){
    const taskName=creation.details?.name;if(!taskName||realizedNames.has(taskName)||creation.__arkdownBuiltInDefinition)continue;
      tasks.push({taskName,group:creation.details?.group||`Customize`,description:creation.details?.description||``,sourcePluginId:creation.details?.sourcePluginId||``}
      )}return tasks}_acceptContextMutation(outcome,{invalidatePlan=false,packageUpdated=false}
  ={}){this._throwBuildScriptDiagnostic(outcome?.diagnostic);
    if(!outcome||!Object.prototype.hasOwnProperty.call(outcome,`value`)){
    throw this._error(`INVALID_HVIGOR_CONTEXT_MUTATION`,`Rust context session returned an invalid mutation`)}
    if(invalidatePlan)this._invalidateRuntimePlan();if(packageUpdated)this._ohpmPackageModelUpdated=true;
    return outcome}_contextValue(outcome){this._throwBuildScriptDiagnostic(outcome?.diagnostic);
    if(!outcome||!Object.prototype.hasOwnProperty.call(outcome,`value`)){
    throw this._error(`INVALID_HVIGOR_CONTEXT_SNAPSHOT`,`Rust context session returned an invalid snapshot`)}
    return outcome.value}_contextPathOptions(){const plan=this._runtimePathPlan();
    return{buildCacheDir:plan.buildCacheDir,buildDir:plan.buildDir,previewMode:plan.previewMode,localTest:plan.localTest,
      ohosTest:plan.ohosTest}}_runtimePathPlan(){return this._runtimeSession.runtimePathPlan(this._parameter.getPropertiesInternal(),
    this._parameter.getExtParamsInternal())}_newEffects(){return{
    compilePluginPath:[],hvigorPluginFile:[],target:null,buildOption:{
      }}}moduleRoots(){const declared=this._moduleDescriptors.map(module=>module.root);
    return declared.length>0?declared:[...this._fallbackModuleRoots]}
  async evaluateHvigorConfig(){this._ensurePublicUtilities();
    if(this._buildConfigFile){this._activeFile=this._buildConfigFile;
      if(this._buildConfigFile.endsWith(`.ts`))this._hvigorTypeScriptRoots.add(this._buildConfigFile);
      await loadModule(this._buildConfigFile,this)}await this._lifecycle.runHook(`configEvaluated`,this._hvigorConfig);
    this._moduleDescriptors=this._runtimeSession.replaceModuleDescriptors(this._hvigorConfig.getAllNodeDescriptorInternal().map(descriptor=>({
    name:descriptor.name,srcPath:descriptor.srcPath,root:path.resolve(this.projectRoot,descriptor.srcPath),targets:descriptor.targets,
      extraOptions:Object.fromEntries(descriptor.extraOptions)}
    )));this._invalidateRuntimePlan()}async runNodesInitialized(){
  await this._lifecycle.runHook(`nodesInitialized`,this._makeHvigorMock(this._activeFile).hvigor)}
  async runTaskGraphResolved(){await this._lifecycle.runHook(`taskGraphResolved`,this._makeHvigorMock(this._activeFile).hvigor)}
  commandPlan(){this._runtimePlan();const response=this._runtimeSession.runtimeCommandPlan({
    executionMode:this._effectiveExecutionMode(),sync:Boolean(this.config.sync),taskNames:this._commandEntryTasks,taskModel:this._runtimeTaskInventory(false)}
    );if(response.error){const{code,message,...details}=response.error;
      throw this._error(code,message,details)}if(this.config.sync){
    this._syncNodeNames=new Set(response.plan.selectedNodeNames)}
    for(const taskPath of new Set([...response.plan.tasks.map(entry=>entry.taskPath),...response.plan.nativeRequests.flatMap(request=>request.rootTaskPaths)])){
    this._commandTaskFromPath(taskPath)}return response.plan}
  _commandTaskFromPath(taskPath){const separator=taskPath.indexOf(`:`);
    const nodeName=separator<0?taskPath:taskPath.slice(0,separator);
    const taskName=separator<0?taskPath:taskPath.slice(separator+1);
    const node=nodeName?this._nodesByName.get(nodeName):Array.from(this._nodesByFile.values()).find(candidate=>candidate.isProject);
    const task=node?this._getTaskByName(node,taskName):undefined;
    if(!task){throw this._error(`ARKDOWN_RUNTIME_TASK_NOT_FOUND`,`Rust command plan references unknown runtime task ${JSON.stringify(taskPath)}`,
      {taskPath})}return task}async executeCommandTask(entry,runRuntimeTasks){
  const{task,commandKind}=entry;let action;if(commandKind===`tasks`){
    action=()=>this._runtimeTaskReport(task.node,`tasks`)}else if(commandKind===`taskTree`){
    action=()=>this._runtimeTaskReport(task.node,`taskTree`)}
    else if(commandKind===`clean`){action=async()=>{const response=await this._runtimeSession.runHvigorClean(task.node.root,
        task.node.isProject);return this._maintenanceOutput(response,`clean`)}
      }else if(commandKind===`prune`){action=()=>this.pruneHvigorCaches()}
    else if(commandKind===`sync`){action=()=>[]}else if(commandKind===`buildInfo`){
    action=()=>{return this._maintenanceOutput(nativeRuntimeBinding.runHvigorBuildInfo({
        projectRoot:this.projectRoot,nodeName:task.node.name,projectTask:task.node.isProject,module:this._parameter.getExtParam(`module`),
          json:this._parameter.getExtParam(`json`)===``,resolve:this._parameter.getExtParam(`buildOption`)===``}
        ),`buildInfo`)}}else if(commandKind===`collectCoverage`){
    action=async()=>{const message=`Generate coverage statistics reports based on instrumentation dotting data`;
        await this._generateCoverageReport(this._parameter.getExtParam(`projectPath`),this._parameter.getExtParam(`reportPath`),
        this._parameter.getExtParam(`coverageFile`));return[message]}
      }else{throw this._error(`INVALID_ARKDOWN_COMMAND_TASK`,`task ${task.path} is not a standalone command task`)}
    this._runtimeBuiltinActions.set(task.path,action);return this._executeRuntimeCommandTasks([task],runRuntimeTasks)}
  async pruneHvigorCaches(hvigorUserHome=getHvigorUserHome()){
  return this._maintenanceOutput(await nativeRuntimeBinding.runHvigorPrune({
    hvigorUserHome}),`prune`)}async _generateCoverageReport(projectPath,reportPath,coverageFile){
  try{const pluginEntry=this.hvigorWorkspaceNodeModules?require.resolve(`@ohos/hvigor-ohos-plugin`,{
      paths:[this.hvigorWorkspaceNodeModules]}):require.resolve(`@ohos/hvigor-ohos-plugin`);
      const pluginRequire=Module.createRequire(pluginEntry);
      const compose=pluginRequire(`@ohos/hvigor-arkts-compose`);
      if(typeof compose.generateReport!==`function`){throw new TypeError(`@ohos/hvigor-arkts-compose does not export generateReport`)}
      await compose.generateReport(projectPath,reportPath,coverageFile)}
    catch(cause){throw this._error(`ARKDOWN_COVERAGE_RUNTIME_NOT_FOUND`,`collectCoverage requires the Hvigor 6.24.3 coverage runtime: ${cause.message}`,
      {cause})}}_maintenanceOutput(response,operation){if(!response||typeof response!==`object`){
    throw this._error(`INVALID_ARKDOWN_MAINTENANCE_RESPONSE`,`Rust ${operation} runner returned an invalid response`,{
      operation})}if(response.error){const{code,message,...details}
      =response.error;throw this._error(code,message,details)}
    if(!Array.isArray(response.output)){throw this._error(`INVALID_ARKDOWN_MAINTENANCE_RESPONSE`,`Rust ${operation} runner returned no output array`,
      {operation})}return response.output}async executeCustomCommandTasks(tasks,runRuntimeTasks){
  if(tasks.length===0)return;await this._executeRuntimeCommandTasks(tasks,runRuntimeTasks)}
  async _executeRuntimeCommandTasks(tasks,runRuntimeTasks){if(typeof runRuntimeTasks!==`function`){
    throw this._error(`ARKDOWN_NATIVE_TASK_RUNTIME_UNAVAILABLE`,`the Rust runtime task engine is required to execute hvigor command tasks`)}
    this._runtimeSession.prepareRuntimeTaskCycle(tasks.map(task=>task.path),tasks.map(task=>task.node.name));
    try{return await runRuntimeTasks(this.createNativeTaskBridge({
      validateBoundaries:false}))}finally{this.clearNativeTaskSelection()}
    }prepareNativeCommandCycle(nodeNames,customRoots,rootTasks){
  this._runtimeSession.prepareCommandTaskCycle(nodeNames,customRoots.map(task=>task.path),rootTasks.map(task=>task.path))}
  prepareDirectBuildCycle(){this._runtimePlan();for(const taskPath of this._runtimeSession.prepareDirectTaskCycle(this._runtimeTaskInventory(false))){
    this._commandTaskFromPath(taskPath)}}createNativeTaskBridge(options={
  }){const runtimeTasks=this._runtimeTaskInventory(false);const selection=this._runtimeSession.runtimeTaskSelection(options.validateBoundaries!==false);
    return{...runtimeTasks,selection,dispatchTask:request=>this._dispatchRuntimeTask(request),commandModifierTypes:Array.from(this._commandBuilderModifiers).filter(([,
      modifier])=>modifier).map(([builderType])=>builderType),transformAbc:async event=>{
      await this._runTransformAbcEvent(event)},modifyNativeCommand:event=>this._runNativeCommandModifier(event)}
    }clearNativeTaskSelection(){this._runtimeSession.clearRuntimeTaskSelection();
    this._runtimeBuiltinActions.clear();this._runtimeTaskFailures.clear()}
  _runtimeTaskReportDescriptors(){return this._runtimeTaskInventory(true).tasks}
  _runtimeTaskInventory(includeCatalog){const nodes=Array.from(this._nodesByFile.values());
    if(includeCatalog)for(const node of nodes)this._syncBuiltinCatalog(node);
    return nativeRuntimeBinding.inspectRuntimeTaskModel(nodes,includeCatalog)}
  _runtimeTaskState(task){return nativeRuntimeBinding.inspectRuntimeTaskState(task.coreWrapper,task.taskContext,task.classIncrementalTask)}
  _runtimeTaskDeclarations(task){return nativeRuntimeBinding.inspectRuntimeTaskDeclarations(task.coreWrapper,task.taskContext,
    task.classIncrementalTask)}async _dispatchRuntimeTask({taskPath,phase,error:message}
  ={}){const capture=async action=>{try{const task=this._commandTaskFromPath(taskPath);
        if(!task)throw this._error(`ARKDOWN_RUNTIME_TASK_NOT_FOUND`,`Rust requested unknown runtime task ${taskPath}`);
        return await action(task)}catch(error){this._runtimeTaskFailures.set(taskPath,error);
        throw error}};if(phase===`failed`){const task=this._commandTaskFromPath(taskPath);
      const cause=this._runtimeTaskFailures.get(taskPath)||new Error(message||`task failed`);
      this._runtimeTaskFailures.delete(taskPath);task.taskContext.onFailed(cause);
      return{}}if(phase===`realize`)return capture(task=>this._runtimeTaskState(task));
    if(phase===`beforeAlways`){return capture(async task=>{await task.coreWrapper.beforeAlwaysAction();
        return this._runtimeTaskDeclarations(task)})}if(phase===`taskShouldDo`){
    return capture(task=>({taskShouldDo:task.taskContext.taskShouldDo()!==false}
      ))}if(phase===`beforeRun`)return capture(async task=>(await this._runTaskBeforeHooks(task),{
    }));if(phase===`afterRun`)return capture(async task=>(await this._runTaskAfterHooks(task),{
    }));if(phase===`run`){return capture(async task=>{if(task.builtIn){
        const action=this._runtimeBuiltinActions.get(task.path);
          if(typeof action!==`function`){throw this._error(`ARKDOWN_BUILTIN_TASK_ACTION_NOT_BOUND`,`Rust requested an unbound standalone built-in task action for ${task.path}`)}
          return{output:toArray(await action())}}const action=task.coreWrapper.getAction();
        if(typeof action!==`function`){throw this._error(`INVALID_HVIGOR_TASK_RUN`,`task ${task.key} must contain a run function`)}
        await action.call(task.coreWrapper);await this._awaitRuntimeTaskPendingPromises(task);
        return{}})}throw this._error(`ARKDOWN_RUNTIME_TASK_PHASE_NOT_FOUND`,`Rust requested unknown runtime task phase ${String(phase)}`)}
  async _awaitRuntimeTaskPendingPromises(task){const pendingPromises=task.coreWrapper.pendingPromises||task.taskContext?.pendingPromises;
    if(!pendingPromises||typeof pendingPromises.get!==`function`)return;
    const pending=toArray(pendingPromises.get());pendingPromises.clear?.();
    await Promise.all(pending)}_runtimeTaskReport(node,reportKind){
  return[nativeRuntimeBinding.runtimeTaskReport({nodeName:node.name,projectTask:node.isProject,kind:node.kind,reportKind,
      tasks:this._runtimeTaskReportDescriptors()})]}async runBuildFinished(error,_nativeReturn=null){
  const utilities=this._ensurePublicUtilities();const reportService=utilities.ReportServiceImpl.getInstance();
    const result=new utilities.BuildResult(error,reportService.getReport());
    try{await this._lifecycle.runHook(`buildFinished`,result)}
    finally{try{try{this._publicUtilities.ProjectCacheService.instance?.close();
          this._publicUtilities.TraceManager.instance||=new this._publicUtilities.TraceManager;
          this._publicUtilities.TraceManager.instance.flush();
          if(this.startParameters().analyze!==2)reportService.report()}
        finally{reportService.stopProcessMonitor()}}finally{
      await this._workerPool?.terminate()}}}_loadModuleDescriptor(descriptor){
  try{const model=this._runtimeSession.loadModuleDescriptor(descriptor);
      return model}catch(cause){throw this._error(`INVALID_HVIGOR_MODEL_FILE`,`failed to load the Rust module runtime model: ${cause.message}`,
      {cause})}}discoverBuildScriptFiles(roots){try{return this._runtimeSession.discoverBuildScriptFiles(toArray(roots))}
    catch(cause){throw this._error(`INVALID_HVIGOR_MODEL_FILE`,`failed to discover build scripts in Rust: ${cause.message}`,
      {cause})}}_throwBuildScriptDiagnostic(diagnostic){if(!diagnostic)return;
    const{code,message,...details}=diagnostic;throw this._error(code,message,details)}
  _clone(value){if(value===undefined){return undefined}return JSON.parse(JSON.stringify(value))}
  _deepFreeze(value){if(!value||typeof value!==`object`||Object.isFrozen(value)){
    return value}for(const child of Object.values(value)){this._deepFreeze(child)}
    return Object.freeze(value)}_freezeClone(value){return this._deepFreeze(this._clone(value))}
  _makeParameter(){let plan;try{plan=this._runtimeSession.parameterPlan()}
    catch(cause){throw this._error(`ARKDOWN_HVIGOR_PARAMETER_PLAN_FAILED`,`failed to materialize the Rust-owned Hvigor Parameter plan: ${cause.message}`,
      {cause})}const core={properties:plan.properties||{},extParams:plan.extParams||{
      },startParams:plan.coreStartParams||{},workspaceDir:plan.workspaceDir}
    ;for(const key of toArray(plan.undefinedPropertyKeys))core.properties[key]=undefined;
    for(const key of toArray(plan.undefinedExtParamKeys))core.extParams[key]=undefined;
    this._coreParameter=core;return{getPropertyInternal:key=>core.properties[key],getPropertiesInternal:()=>core.properties,
      getExtParam:key=>core.extParams[key],getExtParamInternal:key=>core.extParams[key],getExtParamsInternal:()=>core.extParams}
    }_makeHvigorConfig(){const{HvigorConfig}=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/external/core/hvigor-config.js`,
    module,false);const{HvigorNodeDescriptorImpl}=nativeModuleLoad.call(Module,`@ohos/hvigor/src/base/vigor/plugin/impl/hvigor-node-descriptor-impl.js`,
    module,false);const config=Object.create(HvigorConfig.prototype);
    const root=new HvigorNodeDescriptorImpl(path.basename(this.projectRoot),this.projectRoot);
    const child=(name,srcPath,descriptor={})=>{const value=new HvigorNodeDescriptorImpl(name,srcPath,undefined,root);
      value.targets=descriptor.targets;for(const[key,entry]of Object.entries(descriptor.extraOptions||{
      }))value.extraOptions.set(key,entry);return value};config.allChildrenNodeDescriptor=this._moduleDescriptors.map(descriptor=>child(descriptor.name,
    descriptor.srcPath||path.relative(this.projectRoot,descriptor.root),descriptor));
    config.rootNodeDescriptor=root;root.setChildNode(config.allChildrenNodeDescriptor);
    config._includeNode=(name,srcPath,extraOptions)=>{const moduleRoot=path.resolve(this.projectRoot,srcPath||``);
      if(!srcPath||path.isAbsolute(srcPath)||!fs.existsSync(moduleRoot)){
      const code=path.isAbsolute(srcPath||``)?`THE_SRCPATH_IS_NOT_A_RELATIVE_PATH`:`SOME_PATH_NOT_FOUND`;
        throw this._error(code,`module path does not exist: ${String(srcPath)}`)}
      config.allChildrenNodeDescriptor.push(child(name,srcPath,{
      extraOptions}));root.setChildNode(config.allChildrenNodeDescriptor)}
    ;config._excludeNodeByName=name=>{config.allChildrenNodeDescriptor=config.allChildrenNodeDescriptor.filter(entry=>entry.name!==name);
      root.setChildNode(config.allChildrenNodeDescriptor)};return config}
  runtimePropertyPlan(startParams){let outcome;try{outcome=this._runtimeSession.runtimePropertyPlan(this._parameter.getPropertiesInternal(),
      this._parameter.getExtParamsInternal(),startParams||{}
      )}catch(cause){throw this._error(`ARKDOWN_HVIGOR_RUNTIME_PROPERTY_PLAN_FAILED`,`failed to resolve Rust-owned Hvigor runtime properties: ${cause.message}`,
      {cause})}this._throwBuildScriptDiagnostic(outcome.diagnostic);
    if(!outcome.plan){throw this._error(`ARKDOWN_HVIGOR_RUNTIME_PROPERTY_PLAN_FAILED`,`Rust returned no Hvigor runtime property plan`)}
    return outcome.plan}_effectiveBuildMode(){return this._parameter?.getExtParam(`buildMode`)||this.config.mode||this.config.buildMode||`debug`}
  _effectiveExecutionMode(){if(this.config.executionMode)return this.config.executionMode;
    return this._commandEntryTasks.some(task=>task===`test`||task===`onDeviceTest`)?`module`:undefined}
  _error(code,message,extra={}){const error=new Error(message,extra.cause?{
    cause:extra.cause}:undefined);error.code=code;for(const[key,value]of Object.entries(extra)){
    if(key!==`cause`){error[key]=value}}return error}_warn(code,message){
  emitWarningDiagnostic({code,message})}prepareHvigorfiles(files){
  for(const file of files){if(file.endsWith(`.ts`))this._hvigorTypeScriptRoots.add(file)}
    const orderedFiles=[...files].sort((left,right)=>{const leftProject=path.dirname(path.resolve(left))===this.projectRoot?0:1;
      const rightProject=path.dirname(path.resolve(right))===this.projectRoot?0:1;
      return leftProject-rightProject});for(const file of orderedFiles){
    this._nodeForFile(file,true)}const projectNode=this._nodesByFile.get(this._normalizeFile(path.join(this.projectRoot,
    `hvigorfile.ts`)))||Array.from(this._nodesByFile.values()).find(node=>node.root===this.projectRoot);
    if(projectNode){for(const node of this._nodesByFile.values()){
      if(node!==projectNode){node.parent=projectNode;projectNode.children.push(node);
          projectNode.coreWrapper.addSubModule(node.coreWrapper)}
        }this._hvigorCore.initRootProject(projectNode.coreWrapper);
      for(const node of this._nodesByFile.values()){node.wrapper=this._hvigorCore.getHvigorNodeByScriptPath(node.coreWrapper.getBuildFilePath());
        this._bindOfficialNode(node)}}}effectsFor(file){return this._nodeForFile(file,true).effects}
  async configureHvigorfiles(files){if(files.length===0)return[];
    this.prepareHvigorfiles(files);await this.runNodesInitialized();
    if(!files.some(file=>path.dirname(path.resolve(file))===this.projectRoot)){
    return this._configureHvigorfilesWithoutProject(files)}installHvigorTypeScriptExtension();
    const event=this._ensurePublicUtilities().MetricFactory.createDurationEvent(`configuration`,`Configure Hvigor nodes`,
    `configuration`);try{await this.loadWithModuleMocks(this.projectRoot,()=>this._officialConfiguration(event))}
    catch(cause){const node=this._nodeForFile(this._activeFile,false);
      const plugins=toArray(node?.exportValue?.plugins);const failed=plugins.find(plugin=>cause?.message?.includes(plugin?.pluginId))||plugins[0];
      if(failed?.pluginId){throw this._error(`HVIGOR_CUSTOM_PLUGIN_APPLY_FAILED`,`custom plugin ${JSON.stringify(failed.pluginId)} failed while applying to ${node.name}: ${cause.message}`,
        {cause,pluginId:failed.pluginId,hvigorfile:node.file}
        )}throw cause}return files.map(file=>{const value=this._evaluatedBuildScriptValues.get(this._normalizeFile(file));
      const node=this._nodeForFile(file,true);node.loaderConfig=value&&typeof value===`object`?value.config:undefined;
      node.exportValue=value;return{file,value}})}async _configureHvigorfilesWithoutProject(files){
  const records=[];for(const file of files){const node=this._nodeForFile(file,true);
      this._activeFile=node.file;this._effects=node.effects;
      await this._lifecycle.runHook(`beforeNodeEvaluate`,node.wrapper);
      await node.coreWrapper.executeNodeHook(`beforeNodeEvaluate`);
      const value=unwrapDefault(await loadModule(file,this));
      node.loaderConfig=value&&typeof value===`object`?value.config:undefined;
      node.exportValue=value;node.coreWrapper.loadConfig(node.loaderConfig);
      const system=value?.system||value;if(typeof system===`function`){
      const plugin=await system(node.coreWrapper);node.coreWrapper.bindPlugin(plugin);
        node.coreWrapper.bindPluginContextFunc(plugin.getPluginId(),plugin.getContext.bind(plugin))}
      const plugins=toArray(value?.plugins).filter(plugin=>plugin&&typeof plugin.apply===`function`);
      for(const plugin of plugins){if(plugin.context)node.coreWrapper.bindPluginContextFunc(plugin.pluginId,plugin.context.bind(plugin))}
      for(const plugin of plugins){try{await plugin.apply(node.wrapper)}
        catch(cause){throw this._error(`HVIGOR_CUSTOM_PLUGIN_APPLY_FAILED`,`custom plugin ${JSON.stringify(plugin.pluginId)} failed while applying to ${node.name}: ${cause.message}`,
          {cause,pluginId:plugin.pluginId,hvigorfile:file})}
        }await this._lifecycle.runHook(`afterNodeEvaluate`,node.wrapper);
      await node.coreWrapper.executeNodeHook(`afterNodeEvaluate`);
      records.push({file,value})}await this._lifecycle.runHook(`nodesEvaluated`,this._makeHvigorMock(this._activeFile).hvigor);
    return records}_normalizeFile(file){const resolved=path.resolve(file);
    for(const extension of[`.ts`,`.mjs`,`.cjs`,`.js`]){if(resolved.endsWith(extension)){
      return resolved.slice(0,-extension.length)}}return resolved}
  _nodeForFile(file,create=false){const normalized=this._normalizeFile(file);
    let node=this._nodesByFile.get(normalized)||this._nodeAliasesByFile.get(normalized);
    if(node||!create){return node}const actualFile=path.resolve(file);
    const root=path.dirname(actualFile);const isProject=root===this.projectRoot;
    const descriptor=isProject?undefined:this._runtimeSession.moduleDescriptor(root)||this._loadModuleDescriptor({
    name:path.basename(root),srcPath:path.relative(this.projectRoot,root),root}
    );const name=descriptor?.name||(isProject?path.basename(this.projectRoot):path.basename(root));
    node={file:actualFile,root,name,isProject,descriptor,kind:null,parent:null,children:[],effects:this._newEffects(),catalogTaskNames:new Set,
      deletedCatalogTasks:new Set,transformAbcActions:new Map,loaderConfig:undefined,exportValue:undefined,moduleProfile:isProject?this._projectProfile:descriptor.moduleProfile||{
      },moduleJson:isProject?undefined:descriptor.moduleJson,modulePackage:isProject?this._projectPackage:descriptor.modulePackage||{
      }};node.taskCatalog=new HvigorTaskCatalog(this,node);const utilities=this._ensurePublicUtilities();
    const projectNode=Array.from(this._nodesByFile.values()).find(candidate=>candidate.isProject);
    node.coreWrapper=isProject?new utilities.ProjectImpl(name,root):new utilities.ModuleImpl(projectNode?.coreWrapper,name,
    root);node.wrapper=new utilities.HvigorNodeImpl(node.coreWrapper);
    this._runtimeSession.upsertModule(node);this._bindOfficialNode(node);
    for(const[key,value]of Object.entries(descriptor?.extraOptions||{
    }))node.coreWrapper.addExtraOption(key,value);this._initializeCoreTasks(node);
    this._nodesByFile.set(normalized,node);this._nodesByName.set(name,node);
    this._invalidateRuntimePlan();return node}_bindOfficialNode(node){
  const host=this;const core=node.coreWrapper;if(!core.__arkdownHost){
    Object.defineProperties(core,{__arkdownHost:{value:host}
        ,__arkdownNode:{value:node}});const container=core.getTaskContainer();
      const getTaskByName=core.getTaskByName.bind(core);core.getTaskByName=taskName=>{
      host._syncBuiltinCatalog(node);return getTaskByName(taskName)}
      ;const getTaskPaths=core.getTaskPaths.bind(core);core.getTaskPaths=()=>{
      host._syncBuiltinCatalog(node);return getTaskPaths()};
      const getAllTasksCA=core.getAllTasksCA.bind(core);core.getAllTasksCA=()=>{
      host._syncBuiltinCatalog(node);return getAllTasksCA()}
      ;const deleteTask=container.deleteTask.bind(container);
      container.deleteTask=taskName=>{const existed=container.hasTask(taskName);
        const deleted=deleteTask(taskName);if(node.taskCatalog.has(taskName))node.deletedCatalogTasks.add(taskName);
        return existed&&deleted!==false};const registry=core.registry.bind(core);
      core.registry=task=>{const catalogDefinition=node.taskCatalog.get(task.getName());
        const sourcePluginId=task.getSourcePluginId?.();
        if(task.__arkdownBuiltInDefinition||sourcePluginId===`hvigor`||sourcePluginId===catalogDefinition?.sourcePluginId)task.__arkdownBuiltInDefinition||=catalogDefinition||{name:task.getName(),sourcePluginId};
        else task.__arkdownUserTask=true;
        host._ensurePublicUtilities().bindTaskToSession(task,task instanceof host._ensurePublicUtilities().DefaultSyncTask);
        node.deletedCatalogTasks.delete(task.getName());return registry(task)}
      ;const registerTask=core.registerTask.bind(core);core.registerTask=definition=>{
      if(typeof definition?.getName===`function`&&typeof definition?.getAction===`function`){
        core.registry(definition);return definition}const registered=registerTask(definition);
        if(registered&&typeof registered===`object`)registered.__arkdownUserTask=true;return registered}
      ;const task=core.task.bind(core);core.task=(action,taskInfo)=>{
      const registered=task(action,taskInfo);if(registered&&typeof registered===`object`)registered.__arkdownUserTask=true;
        return registered};const bindPluginContextFunc=core.bindPluginContextFunc.bind(core);
      core.bindPluginContextFunc=(pluginId,context)=>{if(pluginId===`__arkdown_config__`)return;
        if(core.getAllPluginIds().includes(pluginId))return;
        return bindPluginContextFunc(pluginId,context)};const bindPlugin=core.bindPlugin.bind(core);
      core.bindPlugin=plugin=>plugin?.__arkdownConfigOnly?plugin:bindPlugin(plugin)}
    const wrapper=node.wrapper;wrapper.getName||=wrapper.getNodeName.bind(wrapper);
    wrapper.getProject||=()=>(node.isProject?node:node.parent)?.wrapper;
    wrapper.findModuleByName||=name=>(node.isProject?node:node.parent)?.children.find(item=>item.name===name)?.wrapper;
    wrapper.getPluginById||=pluginId=>core.getPluginById(pluginId);
    if(!wrapper.__arkdownHost){Object.defineProperties(wrapper,{
      __arkdownHost:{value:host},__arkdownNode:{value:node}}
      )}}_aliasBuildScriptFile(alias,source){const node=this._nodeForFile(source,false);
    if(node){this._nodeAliasesByFile.set(this._normalizeFile(alias),node)}
    }_pluginIdForKind(kind){return{app:`com.ohos.app`,hap:`com.ohos.hap`,har:`com.ohos.har`,hsp:`com.ohos.hsp`}
    [kind]}_moduleTypeForKind(kind){return{hap:`entry`,har:`har`,hsp:`shared`}
    [kind]||kind}_invalidateRuntimePlan(){this._runtimeSession.invalidate()}
  _runtimePlan(){const rawModuleSelection=this._parameter.getExtParam(`module`)??this.config.module;
    const response=this._runtimeSession.plan({productName:this._selectedProductName,buildModeName:this._effectiveBuildMode(),
      moduleSelection:typeof rawModuleSelection===`string`?rawModuleSelection:undefined,properties:this._coreParameter.properties,
      extParams:this._coreParameter.extParams,daemon:Boolean(this.startParameters().daemon),syncVersionChanged:this._syncVersionChanged}
    );if(response.diagnostic){const{code,message,...details}
      =response.diagnostic;throw this._error(code,message,details)}
    return response.plan}_runtimePlanFor(node){this._runtimePlan();
    const module=this._runtimeSession.modulePlan(node.root);
    if(!module){throw this._error(`ARKDOWN_BUILD_SCRIPT_RUNTIME_MODULE_NOT_FOUND`,`Rust build-script runtime plan does not contain module ${JSON.stringify(node.name)} at ${node.root}`,
      {moduleName:node.name})}return module}_targetEntriesFor(node){
  return node.isProject?[{name:`default`}]:this._runtimePlanFor(node).selectedTargets}
  compilePluginRecords(){return this._runtimePlan().compilePluginRecords}
  _selectedProduct(){const product=this._selectedProductDefinition();
    return{getProductName:()=>product.name,getBundleType:()=>product.bundleType||`app`,getBundleName:()=>this._runtimeSession.selectedProductBundleName(),
      getOutputArtifactName:()=>product.output?.artifactName}
    }_selectedProductDefinition(){return this._runtimeSession.selectedProduct()}
  _buildOptionFor(node,target){const plan=this._runtimePlanFor(node);
    return plan.buildOptions[target.name]||plan.buildOptions.default||{
    }}_targetFor(node,target){const paths=this._runtimeSession.moduleTargetPaths(node.root,target.name,this._contextPathOptions());
    return{getTargetName:()=>target.name,getCurrentProduct:()=>this._selectedProduct(),getBuildOption:()=>this._freezeClone(this._buildOptionFor(node,
      target)),getBuildTargetOutputPath:()=>paths.buildTargetOutputPath,getModulePathDetails:()=>({
      getModuleBuildPath:()=>paths.moduleBuildPath,getIntermediatesRes:()=>paths.intermediatesResPath,getModuleBuildCachePath:()=>paths.moduleBuildCachePath}
      )}}_dependencyContextFor(node){return this._contextValue(this._runtimeSession.dependencyContext(node.isProject?undefined:node.root))}
  _setProjectBuildProfile(node,profile){const outcome=this._acceptContextMutation(this._runtimeSession.setProjectBuildProfile(profile),
    {invalidatePlan:true});this._projectProfile=outcome.value;
    node.moduleProfile=outcome.value;this._selectedProductName=outcome.selectedProductName}
  _packageContext(node,includeOverrides=false){const context={
    };const fields=[`Dependencies`,`DevDependencies`,`DynamicDependencies`];
    if(includeOverrides)fields.push(`Overrides`);for(const name of fields){
    const key=`${name[0].toLowerCase()}${name.slice(1)}`;const getter=node.isProject?`projectPackageField`:`modulePackageField`;
      const setter=node.isProject?`setProjectPackageField`:`setModulePackageField`;
      context[`get${name}Opt`.replace(`OverridesOpt`,`Overrides`)]=()=>node.isProject?this._runtimeSession[getter](key):this._runtimeSession[getter](node.root,
      key);context[`set${name}Opt`.replace(`OverridesOpt`,`Overrides`)]=value=>{
      const outcome=this._acceptContextMutation(node.isProject?this._runtimeSession[setter](key,value):this._runtimeSession[setter](node.root,
        key,value),{invalidatePlan:true,packageUpdated:true}
        );if(node.isProject)this._projectPackage=outcome.value;
        else{node.modulePackage=outcome.value}}}return context}
  _targetArtifactNames(node){return new Map(this._targetEntriesFor(node).filter(target=>typeof target.output?.artifactName===`string`).map(target=>[target.name,
    target.output.artifactName]))}_sdkDetails(){if(this._sdkDetailsValue)return this._sdkDetailsValue;
    let inventory;try{inventory=this._runtimeSession.sdkDetails()}
    catch(cause){throw this._error(`INVALID_SDK_COMPONENT_METADATA`,cause.message,{
      cause})}this._sdkDetailsValue={isOhos:()=>inventory.isOhos,getSdkDir:()=>inventory.sdkDir,getSdkVersion:()=>inventory.sdkVersion,
      getEtsComponentVersion:()=>inventory.etsComponentVersion,getEtsComponentReleaseType:()=>inventory.etsComponentReleaseType}
    ;return this._sdkDetailsValue}_ohosContextFor(node){if(node.ohosContext){
    return node.ohosContext}if(node.kind===`app`){const paths=this._runtimeSession.projectPaths(this._contextPathOptions());
      node.ohosContext={...this._packageContext(node,true),getProjectName:()=>node.name,getProjectPath:()=>node.root,getBuildRootPath:()=>paths.buildRootPath,
        getBuildProductOutputPath:()=>paths.buildProductOutputPath,getBuildMode:()=>this._effectiveBuildMode(),getBuildProfileOpt:()=>this._clone(this._projectProfile),
        getCurrentProduct:()=>this._selectedProduct(),getAppJsonOpt:()=>this._clone(this._appJson),getSdkDetails:()=>this._sdkDetails(),
        isDebugLineEnable:()=>String(this._parameter.getExtParam(`ohos-debug-line`))===`true`,getFastBuildApp:()=>Boolean(this._projectProfile.app?.buildOption?.packOptions?.fastBuildApp),
        getAppWithSignedPkg:()=>Boolean(this._projectProfile.app?.buildOption?.packOptions?.appWithSignedPkg),getOhpmDependencyInfo:()=>this._clone(this._dependencyContextFor(node).ohpmDependencyInfo),
        getOhpmRemoteHspDependencyInfo:(isSigned=false)=>this._clone(isSigned?this._dependencyContextFor(node).signedRemoteHspDependencyInfo:this._dependencyContextFor(node).remoteHspDependencyInfo),
        getAllModuleNameHash:()=>this._contextValue(this._runtimeSession.allModuleNameHash()),setBuildProfileOpt:profile=>this._setProjectBuildProfile(node,
        profile),setAppJsonOpt:appJson=>{this._appJson=this._acceptContextMutation(this._runtimeSession.setProjectAppJson(appJson)).value}
        };return node.ohosContext}const artifactNames=this._targetArtifactNames(node);
    node.ohosContext={...this._packageContext(node),getModuleName:()=>node.name,getModulePath:()=>node.root,getModuleType:()=>this._moduleTypeForKind(node.kind),
      getBuildProductRootPath:()=>node.root,getBuildMode:()=>this._effectiveBuildMode(),targets:callback=>this._targetEntriesFor(node).forEach(target=>callback(this._targetFor(node,
      target))),getTargetArtifactNames:()=>new Map(artifactNames),getTargetArtifactName:targetName=>artifactNames.get(targetName),
      getBuildProfileOpt:()=>this._clone(node.moduleProfile),setBuildProfileOpt:profile=>{
      node.moduleProfile=this._acceptContextMutation(this._runtimeSession.setModuleBuildProfile(node.root,profile),{
        invalidatePlan:true}).value;node.taskCatalog.invalidate();
        node.parent?.taskCatalog.invalidate()},getModuleJsonOpt:()=>this._clone(node.moduleJson),setModuleJsonOpt:moduleJson=>{
      node.moduleJson=this._acceptContextMutation(this._runtimeSession.setModuleJson(node.root,moduleJson)).value}
      ,getVersion:()=>this._runtimeSession.modulePackageField(node.root,`version`),getOhpmDependencyInfo:()=>this._clone(this._dependencyContextFor(node).ohpmDependencyInfo),
      getOhpmRemoteHspDependencyInfo:(isSigned=false)=>this._clone(isSigned?this._dependencyContextFor(node).signedRemoteHspDependencyInfo:this._dependencyContextFor(node).remoteHspDependencyInfo),
      setVersion:version=>{node.modulePackage=this._acceptContextMutation(this._runtimeSession.setModuleVersion(node.root,
        version),{invalidatePlan:true,packageUpdated:true}).value}
      ,loadCompilePlugin:pluginPath=>this._loadCompilePlugin(node,pluginPath),transformAbc:(callback,targetName)=>{
      const key=targetName??`all`;node.transformAbcActions.set(key,callback)}
      };return node.ohosContext}_typeCheckCompilePlugin(pluginFile){
  this._typeCheckTypeScript([pluginFile],{baseUrl:this.hvigorWorkspaceNodeModules||path.resolve(process.env.WORKSPACE_DIR||this.projectRoot,
      `node_modules`),skipLibCheck:true,missingMessage:cause=>`type-checking compile plugin ${pluginFile} requires TypeScript ${cause.message}`,
      failureLabel:`compile plugin ${pluginFile}`,details:{pluginPath:pluginFile}
      })}typeCheckBuildScripts(){if(!this.startParameters().hvigorfileTypeCheck||this._hvigorTypeScriptRoots.size===0){
    return}const roots=Array.from(this._hvigorTypeScriptRoots).filter(file=>fs.existsSync(file));
    this._typeCheckTypeScript(roots,{baseUrl:this.hvigorWorkspaceNodeModules||path.resolve(this.projectRoot,`node_modules`),
      skipLibCheck:false,missingMessage:cause=>`type-checking hvigorfiles requires TypeScript: ${cause.message}`,failureLabel:roots.join(`, `),
      details:{files:roots}})}_typeCheckTypeScript(roots,options){
  let ts;try{ts=require("typescript")}catch(cause){throw this._error(`HVIGOR_TYPESCRIPT_NOT_FOUND`,options.missingMessage(cause),
      {cause,...options.details})}const program=ts.createProgram(roots,{
    allowJs:true,alwaysStrict:false,baseUrl:options.baseUrl,declaration:false,esModuleInterop:false,forceConsistentCasingInFileNames:false,
      module:ts.ModuleKind.CommonJS,noEmit:true,noImplicitAny:false,noImplicitReturns:false,skipLibCheck:options.skipLibCheck,
      strict:false,target:ts.ScriptTarget.ES2016,typeRoots:this._typescriptTypeRoots(),types:[]}
    );const diagnostics=ts.getPreEmitDiagnostics(program);if(diagnostics.length===0)return;
    const formatHost={getCanonicalFileName:fileName=>fileName,getCurrentDirectory:()=>process.cwd(),getNewLine:()=>ts.sys.newLine}
    ;const error=this._error(`TYPE_ERRORS`,`TypeScript errors in ${options.failureLabel}:${ts.sys.newLine}${ts.formatDiagnostics(diagnostics,
    formatHost)}`,options.details);error.diagnostics=diagnostics.map(diagnostic=>{
    const location=diagnostic.file&&diagnostic.start!==undefined?diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start):undefined;
      return{category:ts.DiagnosticCategory[diagnostic.category].toLowerCase(),code:diagnostic.code,message:ts.flattenDiagnosticMessageText(diagnostic.messageText,
        ts.sys.newLine),file:diagnostic.file?.fileName,line:location?location.line+1:undefined,column:location?location.character+1:undefined}
      });throw error}_typescriptTypeRoots(){const candidates=[path.join(this.projectRoot,`node_modules`,`@types`)];
    if(this.hvigorWorkspaceNodeModules){try{const hvigorPackage=fs.realpathSync.native(path.join(this.hvigorWorkspaceNodeModules,
        `@ohos`,`hvigor`));candidates.push(path.join(hvigorPackage,`node_modules`,`@types`))}
      catch{}}return candidates.filter(candidate=>fs.existsSync(candidate))}
  _loadCompilePlugin(node,pluginPath){if(typeof pluginPath!==`string`||pluginPath.length===0){
    throw this._error(`EMPTY_COMPILE_PLUGIN_PATH`,`compile plugin path must not be empty`)}
    const resolved=path.isAbsolute(pluginPath)?pluginPath:path.resolve(node.root,pluginPath);
    if(!fs.existsSync(resolved)){throw this._error(`PLUGIN_PATH_NOT_FOUND`,`compile plugin path does not exist: ${resolved}`,
      {pluginPath:resolved})}if(!resolved.endsWith(`.ts`)){throw this._error(`PLUGIN_FILE_MUST_END_WITH_TS`,`compile plugin file must end with .ts: ${resolved}`,
      {pluginPath:resolved})}this._typeCheckCompilePlugin(resolved);
    node.effects.compilePluginPath.push(resolved)}_initializeCoreTasks(node){
  const container=node.coreWrapper.getTaskContainer();for(const name of node.isProject?[`tasks`,`taskTree`,`prune`]:[`tasks`,
    `taskTree`]){if(container.hasTask(name))continue;const definition={
      name,dependencies:[],group:`Help`,description:``};const coreTask=new(this._ensurePublicUtilities()).DefaultTask(node.coreWrapper,
      definition);coreTask.__arkdownBuiltInDefinition=definition;
      container.addTask(coreTask)}}_getTaskByName(node,name){
  this._syncBuiltinCatalog(node);const coreTask=node.coreWrapper.getTaskByName(name);
    if(!coreTask)return undefined;return this._runtimeTask(node,coreTask)}
  _syncBuiltinCatalog(node){const container=node.coreWrapper.getTaskContainer();
    for(const[name,definition]of node.taskCatalog.entries()){
    if(node.deletedCatalogTasks.has(name))continue;const existing=container.getLazyTask(name);
      if(existing?.__arkdownBuiltInDefinition){existing.depends=[...definition.dependencies];
        existing.details={name,group:definition.group,description:definition.description,sourcePluginId:definition.sourcePluginId}
        ;existing.__arkdownBuiltInDefinition=definition;continue}
      if(existing)continue;const creation={provider:()=>{const task=new(this._ensurePublicUtilities()).DefaultTask(node.coreWrapper,
          {name,group:definition.group,description:definition.description,sourcePluginId:definition.sourcePluginId}
          );task.__arkdownBuiltInDefinition=creation.__arkdownBuiltInDefinition;
          return task},depends:[...definition.dependencies],details:{
        name,group:definition.group,description:definition.description,sourcePluginId:definition.sourcePluginId}
        ,dependsDefinedByUser:[],__arkdownBuiltInDefinition:definition}
      ;container.registerTask(creation);node.catalogTaskNames.add(name)}
    }_runtimeTask(node,coreTask){const classIncrementalTask=typeof coreTask.declareInputs===`function`&&typeof coreTask.declareInputFiles===`function`&&typeof coreTask.declareOutputFiles===`function`;
    const taskContext=coreTask.taskContext||coreTask;return{
    key:`${node.name}:${coreTask.getName()}`,path:coreTask.getPath(),node,taskContext,builtIn:Boolean(coreTask.__arkdownBuiltInDefinition),
      classIncrementalTask,coreWrapper:coreTask,get dependencies(){
      return coreTask.getDependsOn()}}}async _runTaskBeforeHooks(task){
  await task.coreWrapper.executeBeforeRun()}async _runTaskAfterHooks(task){
  await task.coreWrapper.executeAfterRun()}async _runTransformAbcEvent(event){
  if(typeof event.moduleName!==`string`||typeof event.targetName!==`string`||typeof event.abcPath!==`string`||!event.config||typeof event.config.isArkGuardEnabled!==`boolean`){
    throw this._error(`INVALID_ARKDOWN_TRANSFORM_ABC_EVENT`,`invalid transformAbc event: ${JSON.stringify(event)}`)}
    const node=this._nodesByName.get(event.moduleName);if(!node){
    throw this._error(`ARKDOWN_TRANSFORM_ABC_NODE_NOT_FOUND`,`transformAbc references unknown node ${JSON.stringify(event.moduleName)}`)}
    const callback=node.transformAbcActions.get(event.targetName)??node.transformAbcActions.get(`all`);
    if(callback===undefined){return true}await Reflect.apply(callback,undefined,[event.abcPath,this._clone(event.config)]);
    return true}_runNativeCommandModifier(event){const modifier=this._commandBuilderModifiers.get(event?.builderType);
    if(!modifier)return event.command;const command=modifier.modify([...event.command]);
    if(!Array.isArray(command)||command.some(argument=>typeof argument!==`string`)){
    throw this._error(`ARKDOWN_INVALID_COMMAND_MODIFIER_RESULT`,`CommandBuilderModifier.modify() must return string[] for builder type ${String(event.builderType)}`,
      {builderType:event.builderType})}return command}_captureTaskEffect(opts,file,kind){
  const node=file?this._nodeForFile(file,true):null;const effects=node?.effects||this._effects;
    const options=opts||{};const ark=options.buildOption?.arkOptions||{
    };for(const candidate of toArray(options.compilePluginPath).concat(toArray(ark.compilePluginPath))){
    effects.compilePluginPath.push(candidate)}for(const candidate of toArray(options.hvigorPluginFile).concat(toArray(ark.hvigorPluginFile))){
    effects.hvigorPluginFile.push(candidate)}if(options.target)effects.target=options.target;
    if(options.buildOption)effects.buildOption=mergeHvigorConfig(effects.buildOption,options.buildOption);
    return this._systemFactory(kind||`hap`,file)}_systemFactory(kind,file){
  const factory=value=>{if(typeof value?.getBuildFilePath!==`function`){
      return this._captureTaskEffect(value,file,kind)}const node=this._nodesByName.get(value.getName())||this._nodeForFile(value.getBuildFilePath(),
      true);node.kind=kind;this._runtimeSession.upsertModule(node);
      this._invalidateRuntimePlan();node.taskCatalog.invalidate();
      node.parent?.taskCatalog.invalidate();const pluginId=this._pluginIdForKind(kind);
      const plugin={getPluginId:()=>pluginId,getContext:()=>this._ohosContextFor(node)}
      ;value.bindPluginContextFunc(pluginId,plugin.getContext);
      return plugin};Object.defineProperty(factory,"__arkdownHvigorSystemKind",{
    value:kind});return factory}_legacySystemFactory(kind,file){
  const factory=()=>{throw this._error(`ARKDOWN_UNSUPPORTED_LEGACY_FA_MODEL`,`legacy${kind[0].toUpperCase()}${kind.slice(1)}Tasks requires the legacy FA/JSBUNDLE pipeline, which arkdown does not support`,
      {kind,file})};Object.defineProperty(factory,"__arkdownHvigorSystemKind",{
    value:`legacy-${kind}`});return factory}_publicNodePath(node){
  if(typeof node?.getNodePath===`function`)return path.resolve(node.getNodePath());
    const nodeDir=node?.getNodeDir?.();if(typeof nodeDir===`string`)return path.resolve(nodeDir);
    if(typeof nodeDir?.getPath===`function`)return path.resolve(nodeDir.getPath());
    throw this._error(`FAILED_TO_GET_REMOTE_HSP_NODE_PATH`,`getRemoteHspObjList expects an Hvigor node`)}
  _getRemoteHspObjList(node){const moduleRoot=this._publicNodePath(node);
    const project=node?.getProject?.();const projectRoot=project?this._publicNodePath(project):this.projectRoot;
    const response=nativeRuntimeBinding.inspectRemoteHspObjects({
    moduleRoot,projectRoot});if(response.error){const{code,message,...details}
      =response.error;throw this._error(code,message,details)}
    return response.objects}_registryCommandModifier(builderType,modifier){
  this._commandBuilderModifiers.set(builderType,modifier)}_submitWorker(publicNode,taskName,workPath,options){
  const nodeName=publicNode?.getNodeNameInternal?.()??publicNode?.getNodeName?.();
    const node=this._nodesByName.get(nodeName);if(!node){throw this._error(`FAILED_TO_SUBMIT_WORKER_MODULE_NOT_FOUND`,`worker node ${String(nodeName)} was not found`)}
    const task=this._getTaskByName(node,taskName);if(!task){
    throw this._error(`FAILED_TO_SUBMIT_WORKER_TASK_NOT_FOUND`,`worker task ${String(taskName)} was not found on node ${node.name}`)}
    if(typeof workPath!==`string`){throw new TypeError(`submitWorker workPath must be a string`)}
    return this._workerPool.submit(task.coreWrapper,workPath,options)}
  _makeOhosPluginMock(file=this._activeFile){const OhosPluginId=Object.freeze({
    OHOS_APP_PLUGIN:`com.ohos.app`,OHOS_HAP_PLUGIN:`com.ohos.hap`,OHOS_HAR_PLUGIN:`com.ohos.har`,OHOS_HSP_PLUGIN:`com.ohos.hsp`}
    );return{OhosPluginId,appTasks:this._systemFactory(`app`,file),hapTasks:this._systemFactory(`hap`,file),hspTasks:this._systemFactory(`hsp`,
      file),harTasks:this._systemFactory(`har`,file),legacyAppTasks:this._legacySystemFactory(`app`,file),legacyHapTasks:this._legacySystemFactory(`hap`,
      file),legacyHarTasks:this._legacySystemFactory(`har`,file),getRemoteHspObjList:node=>this._getRemoteHspObjList(node),
      CommandBuilderType:OHOS_COMMAND_BUILDER_TYPE,registryCommandModifier:(builderType,modifier)=>this._registryCommandModifier(builderType,
      modifier)}}_makeHvigorMock(file=this._activeFile){const host=this;
    const callerFile=file||host._activeFile;const cacheKey=typeof callerFile===`string`&&callerFile.length>0?host._normalizeFile(callerFile):`<unbound>`;
    const cached=host._hvigorExports.get(cacheKey);if(cached)return cached;
    const publicUtilities=host._ensurePublicUtilities();const hvigor=host._hvigorApi||=Object.assign(Object.create(publicUtilities.hvigor),
    {getHvigorConfig:()=>host._hvigorConfig,getParameter:()=>host._parameter,getHvigorUserHome}
    );const exports={...publicUtilities,getNode:scriptPath=>host._nodeForFile(scriptPath||callerFile||host._activeFile,false)?.wrapper,
      getHvigorNode:scriptPath=>host._nodeForFile(scriptPath||callerFile||host._activeFile,false)?.coreWrapper,hvigor,hvigorConfig:host._hvigorConfig,
      submitWorker:(node,taskName,workPath,options)=>host._submitWorker(node,taskName,workPath,options),HVIGOR_PROJECT_WRAPPER_HOME:path.resolve(host.projectRoot,
      `hvigor`),getAlignTarget:()=>host._parameter.getPropertyInternal(`ohos.align.target`),defaultModelRegistry:host._toolingModelRegistry,
      hvigorCore:host._hvigorCore};host._hvigorExports.set(cacheKey,exports);
    return exports}loadWithModuleMocks(file,callback){const originalLoad=Module._load;
    const host=this;Module._load=function arkdownHvigorModuleLoad(request,parent,isMain){
    if(request===`@ohos/hvigor`){return host._makeHvigorMock(parent?.filename||file)}
      if(request===`@ohos/hvigor-ohos-plugin`){const owner=host._nodeForFile(parent?.filename,false)||host._nodeForFile(host._activeFile,
        false);return host._makeOhosPluginMock(owner?.file||parent?.filename||file)}
      const node=typeof request===`string`&&path.isAbsolute(request)?host._nodeForFile(request,false):undefined;
      if(!node)return originalLoad.call(this,request,parent,isMain);
      host._activeFile=node.file;host._effects=node.effects;
      host._aliasBuildScriptFile(Module._resolveFilename(request,parent,isMain),node.file);
      const loaded=originalLoad.call(this,request,parent,isMain);
      const value=unwrapDefault(loaded);node.exportValue=value;
      host._evaluatedBuildScriptValues.set(host._normalizeFile(node.file),value);
      if(value&&typeof value===`object`&&[`plugins`,`system`,`config`].some(key=>Object.hasOwn(value,key))&&(!value.system||typeof value.system===`object`&&Object.keys(value.system).length===0)){
      const system=()=>({__arkdownConfigOnly:true,getPluginId:()=>`__arkdown_config__`,getContext:()=>undefined}
        );if(Object.hasOwn(loaded,`default`))return{...loaded,default:{
          ...value,system}};return{default:{...value,system}
          }}if(loaded&&typeof loaded===`object`&&!Object.hasOwn(loaded,`default`)&&[`plugins`,`system`,`config`,`compilePluginPath`,
      `hvigorPluginFile`,`buildOption`].some(key=>Object.hasOwn(loaded,key))){
      return{default:loaded}}return loaded};let pending=false;
    try{const result=callback();if(result&&typeof result.then===`function`){
      pending=true;return result.finally(()=>{Module._load=originalLoad}
        )}return result}finally{if(!pending)Module._load=originalLoad}
    }}function normalizePluginList(value,baseDir){const plugins=[];
  appendNormalizedPlugins(plugins,unwrapDefault(value),baseDir);
  return applyFunctionOrder(plugins)}function normalizeSortedPluginList(value,_baseDir,pluginFile=`<compile-plugin>`,moduleRoot,
targetName){const exported=unwrapDefault(value);const items=Array.isArray(exported)?exported:[exported];
  const validation=nativeRuntimeBinding.validateHvigorCompilePlugins(items,pluginFile);
  emitPluginPlanDiagnostics(validation);return items.map(plugin=>normalizeHvigorCompilePlugin(plugin,pluginFile,moduleRoot,
  targetName))}class HvigorCompilePluginPlanner{constructor(config={
  }){this.config=config}apply(plugins){const compilePlugins=plugins.filter(plugin=>plugin.__arkdownHvigorCompilePlugin);
    if(compilePlugins.length===0){return plugins}const response=nativeRuntimeBinding.planHvigorCompilePlugins({
    compileMode:this.config.compileMode,preview:Boolean(this.config.preview),target:this.config.target,mode:this.config.mode||this.config.buildMode}
    ,compilePlugins);if(response.error){emitDiagnostic(`error`,response.error)}
    const retained=new Set(response.placements.map(({pluginIndex}
    )=>compilePlugins[pluginIndex]));return plugins.filter(plugin=>!plugin.__arkdownHvigorCompilePlugin||retained.has(plugin))}
  }function applyFunctionOrder(plugins){const response=nativeRuntimeBinding.planRolldownPackagePlugins(plugins);
  emitPluginPlanDiagnostics(response);return response.placements.map(placement=>plugins[placement.pluginIndex])}
function appendNormalizedPlugins(plugins,item,baseDir){const value=unwrapDefault(item);
  if(!value){return}if(typeof value===`string`){return}if(typeof value===`function`){
  if(value.__arkdownHvigorSystemKind)return;appendNormalizedPlugins(plugins,value(),baseDir);
    return}if(Array.isArray(value)){for(const item of value){
    appendNormalizedPlugins(plugins,item,baseDir)}return}if(typeof value===`object`&&hasRollupHook(value)){
  plugins.push(normalizePluginHooks(value));return}if(typeof value===`object`&&value.path){
  return}}function normalizeHvigorCompilePlugin(plugin,pluginFile,moduleRoot,targetName){
const normalized=normalizePluginHooks(plugin,{compilePlugin:true,pluginFile}
  );const internalProperties={__arkdownHvigorCompilePlugin:{
    enumerable:false,value:true},__arkdownHvigorOrder:{enumerable:false,value:plugin.order}
    };if(moduleRoot){internalProperties.__arkdownModuleRoot={
    enumerable:false,value:path.resolve(moduleRoot)}}if(targetName){
  internalProperties.__arkdownTargetName={enumerable:false,value:targetName}
    }Object.defineProperties(normalized,internalProperties);
  return normalized}function normalizePluginHooks(plugin,options={
}){const compilePlugin=options.compilePlugin===true;const pluginFile=options.pluginFile;
  const normalized=compilePlugin?{name:plugin.name}:{...plugin}
  ;let pluginShareContext={};const directHooks=compilePlugin?PLUGIN_DISPATCH_HOOKS.filter(hookName=>hookName!==`resolveDynamicImport`):PLUGIN_DISPATCH_HOOKS;
  const hookNames=directHooks.filter(hookName=>typeof plugin[hookName]===`function`);
  for(const hookName of ALL_PLUGIN_HOOKS)delete normalized[hookName];
  const resultEnvelope=(hookName,result,discardResult)=>{if(discardResult)return{
    };const wrap=value=>{if(value==null)return{};if(hookName===`resolveId`||hookName===`resolveDynamicImport`)return{
      resolveId:value};if(hookName===`load`)return{load:value}
      ;if(hookName===`transform`)return{transform:value};if(hookName===`shouldInvalidCache`)return{
      shouldInvalidCache:value};return{}};return result&&typeof result.then===`function`?Promise.resolve(result).then(wrap):wrap(result)}
  ;const dispatchPluginHook=(request={})=>{if(request.hookName===`seedContext`){
    pluginShareContext=request.share&&typeof request.share===`object`?request.share:{
      };return{}}const hookName=request.hookName;const moduleInfo=request.moduleInfo;
    const hookCtx=moduleInfo?.id?{moduleInfoMap:{[moduleInfo.id]:moduleInfo}
      }:{};let args;switch(hookName){case`buildStart`:args=compilePlugin?[]:[request.buildStartOptions];
      break;case`resolveId`:case`resolveDynamicImport`:args=[request.id,request.importer,request.resolveOptions];
      break;case`load`:args=[request.id];break;case`transform`:args=[request.code,request.id];
      break;case`shouldInvalidCache`:args=[parseShouldInvalidCacheRequest(request.cacheRequest)];
      break;case`moduleParsed`:args=[moduleInfo];break;case`beforeBuildEnd`:args=[undefined];
      break;case`buildEnd`:{const errors=Array.isArray(request.buildEndContext?.errors)?request.buildEndContext.errors:[];
        let error;if(errors.length>0){error=new Error(compilePlugin?`Rolldown build ended with ${errors.length} error(s)`:errors.map(item=>item.message||String(item)).join(`
`));
          error.name=`RolldownError`;error.errors=errors}args=[error];
        break}default:throw new Error(`unknown Rust plugin hook ${String(hookName)}`)}
    try{const result=plugin[hookName].apply(buildPluginContext(hookCtx,pluginShareContext,request.context),args);
      const discardResult=compilePlugin&&[`buildStart`,`moduleParsed`,`buildEnd`].includes(hookName);
      return resultEnvelope(hookName,result,discardResult)}catch(cause){
    if(compilePlugin&&pluginFile&&cause&&typeof cause===`object`){
      cause.message=`${cause.message} ${pluginFile}`}throw cause}
    };Object.defineProperties(normalized,{__arkdownPluginHooks:{
    value:hookNames},__arkdownDispatchPluginHook:{value:dispatchPluginHook}
    });return normalized}function emitPluginPlanDiagnostics(response){
for(const warning of response.warnings||[])emitDiagnostic(`warn`,warning);
  if(response.error)emitDiagnostic(`error`,response.error)}function emitDiagnostic(level,diagnostic){
if(level===`error`){const err=new Error(diagnostic.message);
    err.code=diagnostic.code;if(diagnostic.pluginName)err.pluginName=diagnostic.pluginName;
    throw err}emitWarningDiagnostic(diagnostic)}function emitWarningDiagnostic(diagnostic){
const message=`[arkdown] ${diagnostic.code}: ${diagnostic.message}`;
  process.emitWarning(message,{code:diagnostic.code,type:`ArkdownWarning`}
  )}function reviveOxcPluginAst(envelope){if(!envelope||typeof envelope!==`object`||!envelope.node)return undefined;
  const program=envelope.node;for(const fixPath of envelope.fixes||[]){
  let node=program;for(const key of fixPath)node=node[key];if(node.bigint!==undefined){
    node.value=BigInt(node.bigint)}else if(node.regex){try{node.value=new RegExp(node.regex.pattern,node.regex.flags)}
      catch{}}}return program}function normalizePluginParseError(cause){
const message=cause instanceof Error?cause.message:String(cause);
  const parsed=/PLUGIN_PARSE_ERROR: (.*) \((\d+):(\d+)\) \[pos=(\d+)\]/.exec(message);
  if(!parsed)return cause;const error=new SyntaxError(`${parsed[1]} (${parsed[2]}:${parsed[3]})`);
  error.code=`PLUGIN_PARSE_ERROR`;error.pos=Number(parsed[4]);
  error.loc={line:Number(parsed[2]),column:Number(parsed[3])}
  ;return error}function parsePluginAst(source,options={},rolldownContext){
const nativeOptions={sourceType:options.sourceType||`module`,ranges:options.ranges===true,locations:options.locations===true,
    sourceFile:options.sourceFile,directSourceFile:options.directSourceFile,allowReturnOutsideFunction:options.allowReturnOutsideFunction===true,
    preserveParens:options.preserveParens===true,captureComments:options.onComment!==undefined&&options.onComment!==null,
    ecmaVersion:options.ecmaVersion??`latest`,allowAwaitOutsideFunction:options.allowAwaitOutsideFunction===true,allowSuperOutsideMethod:options.allowSuperOutsideMethod===true,
    allowHashBang:options.allowHashBang,hasOnToken:options.onToken!==undefined&&options.onToken!==null,hasProgram:options.program!==undefined&&options.program!==null}
  ;let result;try{result=rolldownContext&&typeof rolldownContext.parse===`function`?rolldownContext.parse(source,nativeOptions):nativeRuntimeBinding.parsePluginAst(source,
    nativeOptions)}catch(cause){throw normalizePluginParseError(cause)}
  const comments=result.comments||[];if(Array.isArray(options.onComment)){
  options.onComment.push(...comments)}else if(typeof options.onComment===`function`){
  for(const comment of comments){options.onComment.call(options,comment.type===`Block`,comment.value,comment.start,comment.end,
      comment.loc?.start,comment.loc?.end)}}else if(options.onComment!==undefined&&options.onComment!==null){
  throw new TypeError(`onComment must be a function or an array`)}
  return reviveOxcPluginAst(result.ast)}function buildPluginContext(hookCtx,fallbackShare={
},rolldownContext){const share=hookCtx.share||fallbackShare;
  const moduleIds=hookCtx.moduleIds||[];const moduleInfoMap=hookCtx.moduleInfoMap||{
  };const fileNames=hookCtx.fileNames||{};return{share,getModuleIds(){
    return rolldownContext?rolldownContext.getModuleIds().values():moduleIds.values()}
    ,getModuleInfo(id){return rolldownContext?rolldownContext.getModuleInfo(id):moduleInfoMap[id]||null}
    ,parse(source,options={}){return parsePluginAst(source,options,rolldownContext)}
    ,getFileName(referenceId){if(rolldownContext)return rolldownContext.getFileName(referenceId);
      if(Object.hasOwn(fileNames,referenceId))return fileNames[referenceId];
      const error=new Error(`Unable to get file name for unknown reference ID "${referenceId}"`);
      error.code=`FILE_NAME_NOT_FOUND`;throw error}}}function parseShouldInvalidCacheRequest(request){
const parsed=request&&typeof request===`object`?request:{cacheHit:Boolean(request)}
  ;if(parsed&&typeof parsed===`object`&&parsed.__arkdownOxcAst){
  if(parsed.ast===undefined)parsed.ast=reviveOxcPluginAst(parsed.__arkdownOxcAst);
    delete parsed.__arkdownOxcAst}return parsed}function hasRollupHook(plugin){
return Array.from(ALL_PLUGIN_HOOKS).some(hook=>typeof plugin[hook]===`function`)}
function configureHvigorNodePath(projectRoot,plannedWorkspace,plannedNodeModules){
const workspace=plannedWorkspace||path.join(getHvigorUserHome(),`project_caches`,hvigorProjectHash(projectRoot),`workspace`);
  const nodeModules=plannedNodeModules||path.join(workspace,`node_modules`);
  process.env.WORKSPACE_DIR=workspace;const nodePathEntries=(process.env.NODE_PATH||``).split(path.delimiter).filter(Boolean);
  process.env.NODE_PATH=[nodeModules,...nodePathEntries.filter(entry=>entry!==nodeModules)].join(path.delimiter);
  Module._initPaths();installHvigorTypeScriptExtension();return nodeModules}
function getHvigorUserHome(){if(process.env.HVIGOR_USER_HOME&&path.isAbsolute(process.env.HVIGOR_USER_HOME)){
  return process.env.HVIGOR_USER_HOME}return path.resolve(os.homedir(),`.hvigor`)}
function hvigorProjectHash(projectRoot){return nativeRuntimeBinding.hvigorProjectCacheHash(path.resolve(projectRoot))}
function installHvigorTypeScriptExtension(){if(hvigorTypeScriptExtensionInstalled){
  return}const extension=function hvigorTypeScriptExtension(mod,filename){
  mod._compile(compileHvigorScriptSync(filename),filename)};
  Module._extensions[`.ts`]=extension;Module._extensions[`.mjs`]=extension;
  hvigorTypeScriptExtensionInstalled=true}function markHvigorScriptCompileError(error){
if(error&&typeof error===`object`){error.code=`HVIGOR_TYPESCRIPT_DIAGNOSTIC`}
  return error}function compileHvigorScriptSync(filename,sourceMap=false){
try{return nativeRuntimeBinding.compileHvigorScriptSync(filename,sourceMap)}
  catch(error){throw markHvigorScriptCompileError(error)}}async function compileHvigorScript(filename,sourceMap=false){
try{return await nativeRuntimeBinding.compileHvigorScript(filename,sourceMap)}
  catch(error){throw markHvigorScriptCompileError(error)}}async function loadModule(file,host){
const ext=path.extname(file);if(ext===`.ts`||ext===`.mjs`){return loadHvigorScriptModule(file,host)}
  const resolved=require.resolve(file);if(host){host._aliasBuildScriptFile(resolved,file)}
  delete require.cache[resolved];return host?host.loadWithModuleMocks(file,()=>require(resolved)):require(resolved)}
async function loadHvigorScriptModule(file,host){installHvigorTypeScriptExtension();
  const output=await compileHvigorScript(file);const mod=new Module(file,module.parent||module);
  mod.filename=file;mod.paths=Module._nodeModulePaths(path.dirname(file)).concat(Module.globalPaths);
  if(host){host.loadWithModuleMocks(file,()=>mod._compile(output,file))}
  else{mod._compile(output,file)}return mod.exports}function unwrapDefault(value){
if(value&&typeof value===`object`&&`default`in value){return value.default}
  return value}function requireDefaultExport(value,file){if(value&&typeof value===`object`&&`default`in value&&value.default){
  return value.default}const err=new Error(`PLUGIN_FILE_NO_DEFAULT_EXPORT: ${file}`);
  err.code=`PLUGIN_FILE_NO_DEFAULT_EXPORT`;throw err}function toArray(value){
if(value==null){return[]}return Array.isArray(value)?value:[value]}
function mergeHvigorConfig(base,overlay){return nativeRuntimeBinding.mergeHvigorConfig(base,overlay)}
module.exports={HVIGOR_PARITY_VERSION,HvigorRuntimeHost,HvigorCompilePluginPlanner,RuntimePluginLoader,createBuild,createHvigorCommand,
  normalizePluginList,normalizeSortedPluginList,applyFunctionOrder,parseShouldInvalidCacheRequest,buildPluginContext,requireDefaultExport,
  hvigorProjectHash};
