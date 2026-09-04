"use strict";

const fs = require("fs");
const path = require("path");

function fail(message) {
  throw new Error(message);
}

function arkguardLogger(message, _details, level) {
  if (level === "error") {
    fail(message);
  }
}

function sdkLogger() {
  return {
    info() {},
    debug() {},
    warn() {},
    error(...args) {
      fail(args.map(String).join(" "));
    },
  };
}

function registerInsightIntentKeep(whiteListManager, file) {
  whiteListManager.setCurrentCollector(file.originalPath);
  const keepInfo =
    whiteListManager.fileWhiteListInfo?.fileKeepInfo?.arkUIKeepInfo;
  if (!keepInfo) {
    fail(`ArkGuard has no arkUIKeepInfo collector for ${file.originalPath}`);
  }
  for (const name of file.insightIntentGlobalNames) {
    keepInfo.globalNames.add(name);
  }
  for (const name of file.insightIntentPropertyNames) {
    keepInfo.propertyNames.add(name);
  }
}

async function main() {
  const requestPath = process.argv[2];
  const responsePath = process.argv[3];
  if (!requestPath || !responsePath) {
    fail("ArkGuard bytecode bridge requires request and response paths");
  }
  const request = JSON.parse(fs.readFileSync(requestPath, "utf8"));
  process.env.compileTool = "rollup";
  process.env.compileMode = "esmodule";
  process.env.cachePath = request.cachePath;

  const arkguard = require(request.arkguardPath);
  const bytecodeModule = require(path.join(
    request.etsLoaderPath,
    "lib/fast_build/ark_compiler/bytecode_obfuscator.js",
  ));
  const loggerModule = require(path.join(
    request.etsLoaderPath,
    "lib/fast_build/ark_compiler/logger.js",
  ));
  const loaderMain = require(path.join(request.etsLoaderPath, "main.js"));
  Object.assign(loaderMain.projectConfig, {
    allowEtsAnnotations: true,
    cachePath: request.cachePath,
    compileMode: "esmodule",
    entryArrayForObf: request.entryArray,
    modulePath: request.moduleRoot,
    optTryCatchFunc: true,
    projectRootPath: request.projectRoot,
    useTsHar: request.useTsHar,
  });

  const projectConfig = {
    modulePath: request.moduleRoot,
    projectTopDir: request.projectRoot,
    projectRootPath: request.projectRoot,
    projectPath: request.projectPath,
    cachePath: request.cachePath,
    aceModuleBuild: request.aceModuleBuild,
    aceModuleJsonPath: request.aceModuleJsonPath,
    compileMode: "esmodule",
    buildMode: "release",
    runtimeOS: "HarmonyOS",
    compileHar: request.compileHar,
    compileShared: request.compileShared,
    byteCodeHar: false,
    byteCodeHarInfo: request.byteCodeHarInfo,
    compatibleSdkVersion: request.compatibleSdkVersion,
    etsLoaderVersion: request.compileSdkVersion,
    packageDir: request.packageDir,
    useNormalizedOHMUrl: request.useNormalized,
    widgetCompile: false,
    obfuscationOptions: {
      selfConfig: {
        ruleOptions: {
          enable: true,
          rules: request.ruleFiles,
        },
        consumerRules: [],
      },
      dependencies: {
        libraries: [],
        hspLibraries: [],
        hars: request.dependencyRuleFiles,
        hsps: [],
      },
      sdkApis: request.sdkApis,
      obfuscationCacheDir: request.cacheDir,
      exportRulePath: request.exportRulePath,
    },
  };
  if (request.compatibleSdkVersionStage) {
    projectConfig.compatibleSdkVersionStage =
      request.compatibleSdkVersionStage;
  }
  if (request.resourceTablePath) {
    projectConfig.resourceTableTs = request.resourceTablePath;
  }
  const arkProjectConfig = {
    allowEtsAnnotations: true,
    byteCodeHarInfo: request.byteCodeHarInfo,
    entryArrayForObf: request.entryArray,
    entryPackageInfo: request.entryPackageInfo,
    modulePathMap: request.modulePathMap,
  };
  arkguard.initObfuscationConfig(
    projectConfig,
    arkProjectConfig,
    arkguardLogger,
  );
  if (!arkProjectConfig.isBytecodeObfEnabled) {
    fail("ArkGuard rules did not enable bytecode obfuscation");
  }
  const obfuscator = arkProjectConfig.arkObfuscator;
  const merged = arkProjectConfig.obfuscationMergedObConfig;
  if (!obfuscator || !merged || merged.options.disableObfuscation) {
    fail("ArkGuard did not initialize an enabled bytecode obfuscator");
  }

  const log = sdkLogger();
  const share = {
    getLogger() {
      return log;
    },
    throwArkTsCompilerError(error) {
      fail(error && error.toString ? error.toString() : String(error));
    },
  };
  bytecodeModule.BytecodeObfuscator.init(
    { ...share, projectConfig },
    arkProjectConfig,
  );
  const bytecodeObfuscator = bytecodeModule.BytecodeObfuscator.getInstance();
  if (!bytecodeObfuscator) {
    fail("SDK BytecodeObfuscator did not initialize");
  }
  if (request.resourceTablePath) {
    if (!merged.keepSourceOfPaths.includes(request.resourceTablePath)) {
      merged.keepSourceOfPaths.push(request.resourceTablePath);
    }
    bytecodeObfuscator.setKeepSourceOfPaths(
      new Set([request.resourceTablePath]),
    );
  }

  const originalSourcePaths = new Set(
    request.files.map((file) => file.originalPath),
  );
  if (obfuscator.filePathManager) {
    obfuscator.filePathManager.createOrUpdateSourceFilePaths(
      originalSourcePaths,
    );
  }
  const scanOptions = {
    mNameObfuscation: {
      mEnable: true,
      mReservedProperties: [],
      mRenameProperties: merged.options.enablePropertyObfuscation,
      mKeepStringProperty: !merged.options.enableStringPropertyObfuscation,
      mKeepObjectProperty: merged.options.keepObjectProps,
      mEnableAtKeep: merged.options.enableAtKeep,
    },
    mExportObfuscation: merged.options.enableExportObfuscation,
    mKeepFileSourceCode: {
      mKeepSourceOfPaths: new Set(),
      mkeepFilesAndDependencies: new Set(),
    },
  };
  const reserved = arkguard.readProjectPropertiesByCollectedPaths(
    originalSourcePaths,
    scanOptions,
    request.compileHar,
    !bytecodeObfuscator.isDecoratorObfuscationEnabled(),
  );
  if (merged.options.enablePropertyObfuscation) {
    obfuscator.addReservedSetForPropertyObf(reserved);
  }
  if (merged.options.enableExportObfuscation) {
    obfuscator.addReservedSetForDefaultObf(reserved);
  }

  const typescript = require(require.resolve("typescript", {
    paths: [request.arkguardPath],
  }));
  const whiteListManager = arkguard.ProjectCollections.projectWhiteListManager;
  if (!whiteListManager) {
    fail("ArkGuard project white-list manager was not initialized");
  }
  for (const file of request.files) {
    registerInsightIntentKeep(whiteListManager, file);
    const content = fs.readFileSync(file.buildPath, "utf8");
    const sourceFile = typescript.createSourceFile(
      file.originalPath,
      content,
      typescript.ScriptTarget.Latest,
      true,
      typescript.ScriptKind.TS,
    );
    const transformed = typescript.transform(sourceFile, [
      arkguard.collectReservedNameForObf(merged, false),
    ]);
    transformed.dispose();
  }

  const deletedSourcePaths = obfuscator.filePathManager
    ? obfuscator.filePathManager.getDeletedSourceFilePaths()
    : undefined;
  whiteListManager.createOrUpdateWhiteListCaches(deletedSourcePaths);
  bytecodeObfuscator.removeStructProp();

  for (const recordName of request.recordNameWhiteList) {
    bytecodeObfuscator.registerRecordNameWhiteList(recordName);
  }
  const moduleInfos = new Map(
    request.moduleRecords.map((record) => [
      record.sourcePath,
      { recordName: record.recordName },
    ]),
  );

  fs.mkdirSync(request.cachePath, { recursive: true });
  const cacheSourceMapsPath = path.join(request.cachePath, "sourceMaps.map");
  if (request.sourceMapsPath !== cacheSourceMapsPath) {
    fs.copyFileSync(request.sourceMapsPath, cacheSourceMapsPath);
  }
  bytecodeObfuscator.execute(moduleInfos);

  const bytecodeConfig = bytecodeObfuscator.bytecodeObfuscateConfig;
  const nameCachePath =
    bytecodeConfig.obfuscationRules.printNameCache ||
    bytecodeConfig.defaultNameCachePath;
  const result = {
    modulesAbcPath: request.modulesAbcPath,
    configPath: path.join(request.cacheDir, "config.json"),
    nameCachePath,
    sourceMapsPath: cacheSourceMapsPath,
    originAbcPath: bytecodeConfig.abcFilePath,
    obfuscatedAbcPath: bytecodeConfig.obfAbcFilePath,
  };
  fs.writeFileSync(responsePath, JSON.stringify(result));

  arkguard.ProjectCollections.clearProjectWhiteListManager();
  arkguard.clearGlobalCaches();
  bytecodeModule.BytecodeObfuscator.cleanBcObfuscatorObject();
  loggerModule.CommonLogger.destroyInstance();
}

main().catch((error) => {
  process.stderr.write(`${error && error.stack ? error.stack : error}\n`);
  process.exitCode = 1;
});
