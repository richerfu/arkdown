//! Maps `@kit.XxxKit` named imports to their `@ohos:xxx` source module paths.
//!
//! The ArkRuntime's system module registry only knows `@ohos:xxx` module paths.
//! hvigorw's `ets-loader` performs this rewrite before bundling; we replicate it
//! here so the bundled ABC contains resolvable module specifiers.
//!
//! Derived from the OpenHarmony SDK `kit_configs/*.json` declaration files.
//! Each JSON file maps named exports to their `@ohos.*` source `.d.ts`; the
//! `.d.ts` filenames are converted to `@ohos:` colon-notation module paths.

/// Resolve a single named import from a `@kit.XxxKit` specifier to its
/// `@ohos:xxx` source module path.
///
/// Returns `None` if the kit specifier is unknown or the named export has no
/// known mapping (caller should fall back to keeping the original import).
pub fn resolve_kit_import(kit_specifier: &str, named_import: &str) -> Option<&'static str> {
    match (kit_specifier, named_import) {
        // ── @kit.AVSessionKit ───────────────────────────────────────────────────
        ("@kit.AVSessionKit", "AVCastPicker") => Some("@ohos:multimedia.avCastPicker"),
        ("@kit.AVSessionKit", "AVCastPickerColorMode") => {
            Some("@ohos:multimedia.avCastPickerParam")
        }
        ("@kit.AVSessionKit", "AVCastPickerState") => Some("@ohos:multimedia.avCastPickerParam"),
        ("@kit.AVSessionKit", "AVCastPickerStyle") => Some("@ohos:multimedia.avCastPickerParam"),
        ("@kit.AVSessionKit", "AVInputCastPicker") => Some("@ohos:multimedia.avInputCastPicker"),
        ("@kit.AVSessionKit", "avSession") => Some("@ohos:multimedia.avsession"),

        // ── @kit.AbilityKit ─────────────────────────────────────────────────────
        ("@kit.AbilityKit", "Ability") => Some("@ohos:app.ability.Ability"),
        ("@kit.AbilityKit", "AbilityConstant") => Some("@ohos:app.ability.AbilityConstant"),
        ("@kit.AbilityKit", "AbilityLifecycleCallback") => {
            Some("@ohos:app.ability.AbilityLifecycleCallback")
        }
        ("@kit.AbilityKit", "AbilityStage") => Some("@ohos:app.ability.AbilityStage"),
        ("@kit.AbilityKit", "AbilityStartFailureCode") => {
            Some("@ohos:app.ability.CompletionHandlerForAbilityStartCallback")
        }
        ("@kit.AbilityKit", "ActionExtensionAbility") => {
            Some("@ohos:app.ability.ActionExtensionAbility")
        }
        ("@kit.AbilityKit", "AppServiceExtensionAbility") => {
            Some("@ohos:app.ability.AppServiceExtensionAbility")
        }
        ("@kit.AbilityKit", "ApplicationStateChangeCallback") => {
            Some("@ohos:app.ability.ApplicationStateChangeCallback")
        }
        ("@kit.AbilityKit", "AtomicServiceOptions") => {
            Some("@ohos:app.ability.AtomicServiceOptions")
        }
        ("@kit.AbilityKit", "Callee") => Some("@ohos:app.ability.UIAbility"),
        ("@kit.AbilityKit", "CalleeCallback") => Some("@ohos:app.ability.UIAbility"),
        ("@kit.AbilityKit", "Caller") => Some("@ohos:app.ability.UIAbility"),
        ("@kit.AbilityKit", "CheckPackageHasInstalledOptions") => Some("@system:package"),
        ("@kit.AbilityKit", "CheckPackageHasInstalledResponse") => Some("@system:package"),
        ("@kit.AbilityKit", "ChildProcess") => Some("@ohos:app.ability.ChildProcess"),
        ("@kit.AbilityKit", "ChildProcessArgs") => Some("@ohos:app.ability.ChildProcessArgs"),
        ("@kit.AbilityKit", "ChildProcessOptions") => Some("@ohos:app.ability.ChildProcessOptions"),
        ("@kit.AbilityKit", "CompletionHandler") => Some("@ohos:app.ability.CompletionHandler"),
        ("@kit.AbilityKit", "CompletionHandlerForAbilityStartCallback") => {
            Some("@ohos:app.ability.CompletionHandlerForAbilityStartCallback")
        }
        ("@kit.AbilityKit", "CompletionHandlerForAtomicService") => {
            Some("@ohos:app.ability.CompletionHandlerForAtomicService")
        }
        ("@kit.AbilityKit", "Configuration") => Some("@ohos:app.ability.Configuration"),
        ("@kit.AbilityKit", "ConfigurationConstant") => {
            Some("@ohos:app.ability.ConfigurationConstant")
        }
        ("@kit.AbilityKit", "Context") => Some("@ohos:abilityAccessCtrl"),
        ("@kit.AbilityKit", "EmbeddableUIAbility") => Some("@ohos:app.ability.EmbeddableUIAbility"),
        ("@kit.AbilityKit", "EmbeddedUIExtensionAbility") => {
            Some("@ohos:app.ability.EmbeddedUIExtensionAbility")
        }
        ("@kit.AbilityKit", "EnvironmentCallback") => Some("@ohos:app.ability.EnvironmentCallback"),
        ("@kit.AbilityKit", "ErrorCode") => Some("@ohos:ability.errorCode"),
        ("@kit.AbilityKit", "ExtensionAbility") => Some("@ohos:app.ability.ExtensionAbility"),
        ("@kit.AbilityKit", "InsightIntentContext") => {
            Some("@ohos:app.ability.InsightIntentContext")
        }
        ("@kit.AbilityKit", "InsightIntentEntity") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "InsightIntentEntry") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "InsightIntentEntryExecutor") => {
            Some("@ohos:app.ability.InsightIntentEntryExecutor")
        }
        ("@kit.AbilityKit", "InsightIntentExecutor") => {
            Some("@ohos:app.ability.InsightIntentExecutor")
        }
        ("@kit.AbilityKit", "InsightIntentForm") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "InsightIntentFunction") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "InsightIntentFunctionMethod") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "InsightIntentLink") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "InsightIntentPage") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "LinkParamCategory") => {
            Some("@ohos:app.ability.InsightIntentDecorator")
        }
        ("@kit.AbilityKit", "OnReleaseCallback") => Some("@ohos:app.ability.UIAbility"),
        ("@kit.AbilityKit", "OnRemoteStateChangeCallback") => Some("@ohos:app.ability.UIAbility"),
        ("@kit.AbilityKit", "OpenLinkOptions") => Some("@ohos:app.ability.OpenLinkOptions"),
        ("@kit.AbilityKit", "Package") => Some("@system:package"),
        ("@kit.AbilityKit", "PermissionRequestResult") => Some("@ohos:abilityAccessCtrl"),
        ("@kit.AbilityKit", "Permissions") => Some("@ohos:abilityAccessCtrl"),
        ("@kit.AbilityKit", "PhotoEditorExtensionAbility") => {
            Some("@ohos:app.ability.PhotoEditorExtensionAbility")
        }
        ("@kit.AbilityKit", "ShareExtensionAbility") => {
            Some("@ohos:app.ability.ShareExtensionAbility")
        }
        ("@kit.AbilityKit", "StartOptions") => Some("@ohos:app.ability.StartOptions"),
        ("@kit.AbilityKit", "StartupConfig") => Some("@ohos:app.appstartup.StartupConfig"),
        ("@kit.AbilityKit", "StartupConfigEntry") => {
            Some("@ohos:app.appstartup.StartupConfigEntry")
        }
        ("@kit.AbilityKit", "StartupListener") => Some("@ohos:app.appstartup.StartupListener"),
        ("@kit.AbilityKit", "StartupTask") => Some("@ohos:app.appstartup.StartupTask"),
        ("@kit.AbilityKit", "UIAbility") => Some("@ohos:app.ability.UIAbility"),
        ("@kit.AbilityKit", "UIExtensionAbility") => Some("@ohos:app.ability.UIExtensionAbility"),
        ("@kit.AbilityKit", "UIExtensionContentSession") => {
            Some("@ohos:app.ability.UIExtensionContentSession")
        }
        ("@kit.AbilityKit", "Want") => Some("@ohos:app.ability.Want"),
        ("@kit.AbilityKit", "WantAgent") => Some("@ohos:app.ability.wantAgent"),
        ("@kit.AbilityKit", "ability") => Some("@ohos:ability.ability"),
        ("@kit.AbilityKit", "abilityAccessCtrl") => Some("@ohos:abilityAccessCtrl"),
        ("@kit.AbilityKit", "abilityManager") => Some("@ohos:app.ability.abilityManager"),
        ("@kit.AbilityKit", "appManager") => Some("@ohos:app.ability.appManager"),
        ("@kit.AbilityKit", "appRecovery") => Some("@ohos:app.ability.appRecovery"),
        ("@kit.AbilityKit", "application") => Some("@ohos:app.ability.application"),
        ("@kit.AbilityKit", "autoFillManager") => Some("@ohos:app.ability.autoFillManager"),
        ("@kit.AbilityKit", "autoStartupManager") => Some("@ohos:app.ability.autoStartupManager"),
        ("@kit.AbilityKit", "bundle") => Some("@ohos:bundle"),
        ("@kit.AbilityKit", "bundleManager") => Some("@ohos:bundle.bundleManager"),
        ("@kit.AbilityKit", "childProcessManager") => Some("@ohos:app.ability.childProcessManager"),
        ("@kit.AbilityKit", "common") => Some("@ohos:app.ability.common"),
        ("@kit.AbilityKit", "contextConstant") => Some("@ohos:app.ability.contextConstant"),
        ("@kit.AbilityKit", "continuationManager") => {
            Some("@ohos:continuation.continuationManager")
        }
        ("@kit.AbilityKit", "continueManager") => Some("@ohos:app.ability.continueManager"),
        ("@kit.AbilityKit", "dataUriUtils") => Some("@ohos:app.ability.dataUriUtils"),
        ("@kit.AbilityKit", "defaultAppManager") => Some("@ohos:bundle.defaultAppManager"),
        ("@kit.AbilityKit", "dialogRequest") => Some("@ohos:app.ability.dialogRequest"),
        ("@kit.AbilityKit", "errorManager") => Some("@ohos:app.ability.errorManager"),
        ("@kit.AbilityKit", "featureAbility") => Some("@ohos:ability.featureAbility"),
        ("@kit.AbilityKit", "insightIntent") => Some("@ohos:app.ability.insightIntent"),
        ("@kit.AbilityKit", "kioskManager") => Some("@ohos:app.ability.kioskManager"),
        ("@kit.AbilityKit", "launcherBundleManager") => Some("@ohos:bundle.launcherBundleManager"),
        ("@kit.AbilityKit", "overlay") => Some("@ohos:bundle.overlay"),
        ("@kit.AbilityKit", "particleAbility") => Some("@ohos:ability.particleAbility"),
        ("@kit.AbilityKit", "privacyManager") => Some("@ohos:privacyManager"),
        ("@kit.AbilityKit", "screenLockFileManager") => Some("@ohos:ability.screenLockFileManager"),
        ("@kit.AbilityKit", "sendableContextManager") => {
            Some("@ohos:app.ability.sendableContextManager")
        }
        ("@kit.AbilityKit", "shortcutManager") => Some("@ohos:bundle.shortcutManager"),
        ("@kit.AbilityKit", "startupManager") => Some("@ohos:app.appstartup.startupManager"),
        ("@kit.AbilityKit", "uriPermissionManager") => {
            Some("@ohos:application.uriPermissionManager")
        }
        ("@kit.AbilityKit", "wantAgent") => Some("@ohos:app.ability.wantAgent"),
        ("@kit.AbilityKit", "wantConstant") => Some("@ohos:app.ability.wantConstant"),

        // ── @kit.AccessibilityKit ───────────────────────────────────────────────
        ("@kit.AccessibilityKit", "AccessibilityElement") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "AccessibilityEvent") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "AccessibilityExtensionAbility") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "AccessibilityExtensionContext") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "ElementAttributeKeys") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "ElementAttributeValues") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "FocusDirection") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "FocusType") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "GesturePath") => Some("@ohos:accessibility.GesturePath"),
        ("@kit.AccessibilityKit", "GesturePoint") => Some("@ohos:accessibility.GesturePoint"),
        ("@kit.AccessibilityKit", "Rect") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "WindowType") => {
            Some("@ohos:application.AccessibilityExtensionAbility")
        }
        ("@kit.AccessibilityKit", "accessibility") => Some("@ohos:accessibility"),

        // ── @kit.AdsKit ─────────────────────────────────────────────────────────
        ("@kit.AdsKit", "AdComponent") => Some("@ohos:advertising.AdComponent"),
        ("@kit.AdsKit", "AutoAdComponent") => Some("@ohos:advertising.AutoAdComponent"),
        ("@kit.AdsKit", "RespCallback") => Some("@ohos:advertising.AdsServiceExtensionAbility"),
        ("@kit.AdsKit", "advertising") => Some("@ohos:advertising"),
        ("@kit.AdsKit", "identifier") => Some("@ohos:identifier.oaid"),

        // ── @kit.ArkData ────────────────────────────────────────────────────────
        ("@kit.ArkData", "ContentFormCard") => Some("@ohos:data.UdmfComponents"),
        ("@kit.ArkData", "FormType") => Some("@ohos:data.UdmfComponents"),
        ("@kit.ArkData", "ValueType") => Some("@ohos:data.ValuesBucket"),
        ("@kit.ArkData", "ValuesBucket") => Some("@ohos:data.ValuesBucket"),
        ("@kit.ArkData", "cloudData") => Some("@ohos:data.cloudData"),
        ("@kit.ArkData", "cloudExtension") => Some("@ohos:data.cloudExtension"),
        ("@kit.ArkData", "commonType") => Some("@ohos:data.commonType"),
        ("@kit.ArkData", "dataAbility") => Some("@ohos:data.dataAbility"),
        ("@kit.ArkData", "dataShare") => Some("@ohos:data.dataShare"),
        ("@kit.ArkData", "dataSharePredicates") => Some("@ohos:data.dataSharePredicates"),
        ("@kit.ArkData", "distributedDataObject") => Some("@ohos:data.distributedDataObject"),
        ("@kit.ArkData", "distributedKVStore") => Some("@ohos:data.distributedKVStore"),
        ("@kit.ArkData", "intelligence") => Some("@ohos:data.intelligence"),
        ("@kit.ArkData", "preferences") => Some("@ohos:data.preferences"),
        ("@kit.ArkData", "relationalStore") => Some("@ohos:data.relationalStore"),
        ("@kit.ArkData", "sendablePreferences") => Some("@ohos:data.sendablePreferences"),
        ("@kit.ArkData", "sendableRelationalStore") => Some("@ohos:data.sendableRelationalStore"),
        ("@kit.ArkData", "unifiedDataChannel") => Some("@ohos:data.unifiedDataChannel"),
        ("@kit.ArkData", "uniformDataStruct") => Some("@ohos:data.uniformDataStruct"),
        ("@kit.ArkData", "uniformTypeDescriptor") => Some("@ohos:data.uniformTypeDescriptor"),

        // ── @kit.ArkGraphics2D ──────────────────────────────────────────────────
        ("@kit.ArkGraphics2D", "colorSpaceManager") => Some("@ohos:graphics.colorSpaceManager"),
        ("@kit.ArkGraphics2D", "common2D") => Some("@ohos:graphics.common2D"),
        ("@kit.ArkGraphics2D", "displaySync") => Some("@ohos:graphics.displaySync"),
        ("@kit.ArkGraphics2D", "drawing") => Some("@ohos:graphics.drawing"),
        ("@kit.ArkGraphics2D", "effectKit") => Some("@ohos:effectKit"),
        ("@kit.ArkGraphics2D", "hdrCapability") => Some("@ohos:graphics.hdrCapability"),
        ("@kit.ArkGraphics2D", "sendableColorSpaceManager") => {
            Some("@ohos:graphics.sendableColorSpaceManager")
        }
        ("@kit.ArkGraphics2D", "text") => Some("@ohos:graphics.text"),
        ("@kit.ArkGraphics2D", "uiEffect") => Some("@ohos:graphics.uiEffect"),

        // ── @kit.ArkGraphics3D ──────────────────────────────────────────────────
        ("@kit.ArkGraphics3D", _) => Some("@ohos:graphics.scene"),

        // ── @kit.ArkTS ──────────────────────────────────────────────────────────
        ("@kit.ArkTS", "ArkTSUtils") => Some("@arkts:utils"),
        ("@kit.ArkTS", "ArrayList") => Some("@ohos:util.ArrayList"),
        ("@kit.ArkTS", "Decimal") => Some("@arkts:math.Decimal"),
        ("@kit.ArkTS", "DedicatedWorkerGlobalScope") => Some("@ohos:worker"),
        ("@kit.ArkTS", "Deque") => Some("@ohos:util.Deque"),
        ("@kit.ArkTS", "ErrorEvent") => Some("@ohos:worker"),
        ("@kit.ArkTS", "Event") => Some("@ohos:worker"),
        ("@kit.ArkTS", "EventListener") => Some("@ohos:worker"),
        ("@kit.ArkTS", "EventTarget") => Some("@ohos:worker"),
        ("@kit.ArkTS", "HashMap") => Some("@ohos:util.HashMap"),
        ("@kit.ArkTS", "HashSet") => Some("@ohos:util.HashSet"),
        ("@kit.ArkTS", "JSON") => Some("@ohos:util.json"),
        ("@kit.ArkTS", "LightWeightMap") => Some("@ohos:util.LightWeightMap"),
        ("@kit.ArkTS", "LightWeightSet") => Some("@ohos:util.LightWeightSet"),
        ("@kit.ArkTS", "LinkedList") => Some("@ohos:util.LinkedList"),
        ("@kit.ArkTS", "List") => Some("@ohos:util.List"),
        ("@kit.ArkTS", "MessageEvent") => Some("@ohos:worker"),
        ("@kit.ArkTS", "MessageEvents") => Some("@ohos:worker"),
        ("@kit.ArkTS", "PlainArray") => Some("@ohos:util.PlainArray"),
        ("@kit.ArkTS", "PostMessageOptions") => Some("@ohos:worker"),
        ("@kit.ArkTS", "Queue") => Some("@ohos:util.Queue"),
        ("@kit.ArkTS", "Stack") => Some("@ohos:util.Stack"),
        ("@kit.ArkTS", "ThreadWorkerGlobalScope") => Some("@ohos:worker"),
        ("@kit.ArkTS", "ThreadWorkerPriority") => Some("@ohos:worker"),
        ("@kit.ArkTS", "TreeMap") => Some("@ohos:util.TreeMap"),
        ("@kit.ArkTS", "TreeSet") => Some("@ohos:util.TreeSet"),
        ("@kit.ArkTS", "Vector") => Some("@ohos:util.Vector"),
        ("@kit.ArkTS", "WorkerEventListener") => Some("@ohos:worker"),
        ("@kit.ArkTS", "WorkerEventTarget") => Some("@ohos:worker"),
        ("@kit.ArkTS", "WorkerOptions") => Some("@ohos:worker"),
        ("@kit.ArkTS", "buffer") => Some("@ohos:buffer"),
        ("@kit.ArkTS", "collections") => Some("@arkts:collections"),
        ("@kit.ArkTS", "convertxml") => Some("@ohos:convertxml"),
        ("@kit.ArkTS", "fastbuffer") => Some("@ohos:fastbuffer"),
        ("@kit.ArkTS", "lang") => Some("@arkts:lang"),
        ("@kit.ArkTS", "process") => Some("@ohos:process"),
        ("@kit.ArkTS", "stream") => Some("@ohos:util.stream"),
        ("@kit.ArkTS", "taskpool") => Some("@ohos:taskpool"),
        ("@kit.ArkTS", "uri") => Some("@ohos:uri"),
        ("@kit.ArkTS", "url") => Some("@ohos:url"),
        ("@kit.ArkTS", "util") => Some("@ohos:util"),
        ("@kit.ArkTS", "worker") => Some("@ohos:worker"),
        ("@kit.ArkTS", "xml") => Some("@ohos:xml"),

        // ── @kit.ArkUI ──────────────────────────────────────────────────────────
        ("@kit.ArkUI", "AccessibilityOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "AccessibilitySelectedType") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "AddFormMenuItem") => Some("@ohos:arkui.advanced.FormMenu"),
        ("@kit.ArkUI", "AddFormOptions") => Some("@ohos:arkui.advanced.FormMenu"),
        ("@kit.ArkUI", "AdvancedDialogV2Button") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "AdvancedDialogV2ButtonAction") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "AdvancedDialogV2ButtonOptions") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "AdvancedDialogV2OnCheckedChange") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "AlertDialog") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "AlertDialogV2") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "AlphabetIndexerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "AnimatedDrawableDescriptor") => Some("@ohos:arkui.drawableDescriptor"),
        ("@kit.ArkUI", "AnimationController") => Some("@ohos:arkui.drawableDescriptor"),
        ("@kit.ArkUI", "AnimationOptions") => Some("@ohos:arkui.drawableDescriptor"),
        ("@kit.ArkUI", "Animator") => Some("@ohos:animator"),
        ("@kit.ArkUI", "AnimatorOptions") => Some("@ohos:animator"),
        ("@kit.ArkUI", "AnimatorResult") => Some("@ohos:animator"),
        ("@kit.ArkUI", "App") => Some("@system:app"),
        ("@kit.ArkUI", "AppResponse") => Some("@system:app"),
        ("@kit.ArkUI", "AppStorageV2") => Some("@ohos:arkui.StateManagement"),
        ("@kit.ArkUI", "ArcAlphabetIndexer") => Some("@ohos:arkui.ArcAlphabetIndexer"),
        ("@kit.ArkUI", "ArcAlphabetIndexerAttribute") => Some("@ohos:arkui.ArcAlphabetIndexer"),
        ("@kit.ArkUI", "ArcButton") => Some("@ohos:arkui.advanced.ArcButton"),
        ("@kit.ArkUI", "ArcButtonOptions") => Some("@ohos:arkui.advanced.ArcButton"),
        ("@kit.ArkUI", "ArcButtonPosition") => Some("@ohos:arkui.advanced.ArcButton"),
        ("@kit.ArkUI", "ArcButtonStatus") => Some("@ohos:arkui.advanced.ArcButton"),
        ("@kit.ArkUI", "ArcButtonStyleMode") => Some("@ohos:arkui.advanced.ArcButton"),
        ("@kit.ArkUI", "ArcDirection") => Some("@ohos:arkui.ArcSwiper"),
        ("@kit.ArkUI", "ArcDotIndicator") => Some("@ohos:arkui.ArcSwiper"),
        ("@kit.ArkUI", "ArcList") => Some("@ohos:arkui.ArcList"),
        ("@kit.ArkUI", "ArcListAttribute") => Some("@ohos:arkui.ArcList"),
        ("@kit.ArkUI", "ArcListItem") => Some("@ohos:arkui.ArcList"),
        ("@kit.ArkUI", "ArcListItemAttribute") => Some("@ohos:arkui.ArcList"),
        ("@kit.ArkUI", "ArcScrollBar") => Some("@ohos:arkui.ArcScrollBar"),
        ("@kit.ArkUI", "ArcScrollBarAttribute") => Some("@ohos:arkui.ArcScrollBar"),
        ("@kit.ArkUI", "ArcSlider") => Some("@ohos:arkui.advanced.ArcSlider"),
        ("@kit.ArkUI", "ArcSliderLayoutOptions") => Some("@ohos:arkui.advanced.ArcSlider"),
        ("@kit.ArkUI", "ArcSliderLayoutOptionsConstructorOptions") => {
            Some("@ohos:arkui.advanced.ArcSlider")
        }
        ("@kit.ArkUI", "ArcSliderOptions") => Some("@ohos:arkui.advanced.ArcSlider"),
        ("@kit.ArkUI", "ArcSliderOptionsConstructorOptions") => {
            Some("@ohos:arkui.advanced.ArcSlider")
        }
        ("@kit.ArkUI", "ArcSliderPosition") => Some("@ohos:arkui.advanced.ArcSlider"),
        ("@kit.ArkUI", "ArcSliderStyleOptions") => Some("@ohos:arkui.advanced.ArcSlider"),
        ("@kit.ArkUI", "ArcSliderStyleOptionsConstructorOptions") => {
            Some("@ohos:arkui.advanced.ArcSlider")
        }
        ("@kit.ArkUI", "ArcSliderValueOptions") => Some("@ohos:arkui.advanced.ArcSlider"),
        ("@kit.ArkUI", "ArcSliderValueOptionsConstructorOptions") => {
            Some("@ohos:arkui.advanced.ArcSlider")
        }
        ("@kit.ArkUI", "ArcSwiper") => Some("@ohos:arkui.ArcSwiper"),
        ("@kit.ArkUI", "ArcSwiperAttribute") => Some("@ohos:arkui.ArcSwiper"),
        ("@kit.ArkUI", "ArcSwiperController") => Some("@ohos:arkui.ArcSwiper"),
        ("@kit.ArkUI", "AtomicServiceBar") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "AtomicServiceNavigation") => {
            Some("@ohos:atomicservice.AtomicServiceNavigation")
        }
        ("@kit.ArkUI", "AtomicServiceSearch") => Some("@ohos:atomicservice.AtomicServiceSearch"),
        ("@kit.ArkUI", "AtomicServiceTabs") => Some("@ohos:atomicservice.AtomicServiceTabs"),
        ("@kit.ArkUI", "AtomicServiceWeb") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "AtomicServiceWebController") => {
            Some("@ohos:atomicservice.AtomicServiceWeb")
        }
        ("@kit.ArkUI", "AttributeUpdater") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "BackRouterOptions") => Some("@system:router"),
        ("@kit.ArkUI", "BackgroundTheme") => Some("@ohos:atomicservice.AtomicServiceNavigation"),
        ("@kit.ArkUI", "BasicPrefetcher") => Some("@ohos:arkui.Prefetcher"),
        ("@kit.ArkUI", "Binding") => Some("@ohos:arkui.StateManagement"),
        ("@kit.ArkUI", "BlankModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "BorderRadiusMode") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "BottomOffset") => Some("@ohos:atomicservice.InterstitialDialogAction"),
        ("@kit.ArkUI", "BuildOptions") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "BuilderNode") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "Button") => Some("@system:prompt"),
        ("@kit.ArkUI", "ButtonModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ButtonOptions") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "CalendarPickerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "CallbackParam") => Some("@ohos:arkui.advanced.TreeView"),
        ("@kit.ArkUI", "CapsuleSegmentButtonConstructionOptions") => {
            Some("@ohos:arkui.advanced.SegmentButton")
        }
        ("@kit.ArkUI", "CapsuleSegmentButtonOptions") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "CapsuleSegmentButtonV2") => Some("@ohos:arkui.advanced.SegmentButtonV2"),
        ("@kit.ArkUI", "CheckboxGroupModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "CheckboxModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Chip") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "ChipGroup") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "ChipGroupItemOptions") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "ChipGroupSpaceOptions") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "ChipItemLabelOptions") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "ChipItemStyle") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "ChipOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "ChipSize") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "ChipSuffixSymbolGlyphOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "ChipSymbolGlyphOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "CircleShape") => Some("@ohos:arkui.shape"),
        ("@kit.ArkUI", "CloseOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "ColorMetrics") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "Colors") => Some("@ohos:arkui.theme"),
        ("@kit.ArkUI", "ColumnModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ColumnSplitModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "CommonModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "CommonSegmentButtonOptions") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "ComponentContent") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "ComponentUtils") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "ComposeListItem") => Some("@ohos:arkui.advanced.ComposeListItem"),
        ("@kit.ArkUI", "ComposeTitleBar") => Some("@ohos:arkui.advanced.ComposeTitleBar"),
        ("@kit.ArkUI", "ComposeTitleBarMenuItem") => Some("@ohos:arkui.advanced.ComposeTitleBar"),
        ("@kit.ArkUI", "Configuration") => Some("@system:configuration"),
        ("@kit.ArkUI", "ConfirmDialog") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "ConfirmDialogV2") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "ConnectOptions") => Some("@ohos:arkui.StateManagement"),
        ("@kit.ArkUI", "ContainerSpanModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Content") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "ContentItem") => Some("@ohos:arkui.advanced.ComposeListItem"),
        ("@kit.ArkUI", "ContextMenuController") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "CounterComponent") => Some("@ohos:arkui.advanced.Counter"),
        ("@kit.ArkUI", "CounterModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "CounterOptions") => Some("@ohos:arkui.advanced.Counter"),
        ("@kit.ArkUI", "CounterType") => Some("@ohos:arkui.advanced.Counter"),
        ("@kit.ArkUI", "CursorController") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "CustomColors") => Some("@ohos:arkui.theme"),
        ("@kit.ArkUI", "CustomContentDialog") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "CustomContentDialogV2") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "CustomDarkColors") => Some("@ohos:arkui.theme"),
        ("@kit.ArkUI", "CustomTheme") => Some("@ohos:arkui.theme"),
        ("@kit.ArkUI", "DataPanelModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "DateData") => Some("@ohos:arkui.advanced.Counter"),
        ("@kit.ArkUI", "DatePickerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "DimensionNoPercentage") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "DisableAlertBeforeBackPageOptions") => Some("@system:router"),
        ("@kit.ArkUI", "DividerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "DownloadContentOptions") => Some("@ohos:arkui.advanced.DownloadFileButton"),
        ("@kit.ArkUI", "DownloadDescription") => Some("@ohos:arkui.advanced.DownloadFileButton"),
        ("@kit.ArkUI", "DownloadFileButton") => Some("@ohos:arkui.advanced.DownloadFileButton"),
        ("@kit.ArkUI", "DownloadIconStyle") => Some("@ohos:arkui.advanced.DownloadFileButton"),
        ("@kit.ArkUI", "DownloadLayoutDirection") => {
            Some("@ohos:arkui.advanced.DownloadFileButton")
        }
        ("@kit.ArkUI", "DownloadStyleOptions") => Some("@ohos:arkui.advanced.DownloadFileButton"),
        ("@kit.ArkUI", "DragController") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "DrawContext") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "DrawableDescriptor") => Some("@ohos:arkui.drawableDescriptor"),
        ("@kit.ArkUI", "DrawableDescriptorLoadedResult") => Some("@ohos:arkui.drawableDescriptor"),
        ("@kit.ArkUI", "EditableLeftIconType") => Some("@ohos:arkui.advanced.EditableTitleBar"),
        ("@kit.ArkUI", "EditableTitleBar") => Some("@ohos:arkui.advanced.EditableTitleBar"),
        ("@kit.ArkUI", "EditableTitleBarItem") => Some("@ohos:arkui.advanced.EditableTitleBar"),
        ("@kit.ArkUI", "EditableTitleBarMenuItem") => Some("@ohos:arkui.advanced.EditableTitleBar"),
        ("@kit.ArkUI", "EditableTitleBarOptions") => Some("@ohos:arkui.advanced.EditableTitleBar"),
        ("@kit.ArkUI", "EditorEventInfo") => Some("@ohos:arkui.advanced.SelectionMenu"),
        ("@kit.ArkUI", "EditorMenuOptions") => Some("@ohos:arkui.advanced.SelectionMenu"),
        ("@kit.ArkUI", "EllipseShape") => Some("@ohos:arkui.shape"),
        ("@kit.ArkUI", "EnableAlertBeforeBackPageOptions") => Some("@system:router"),
        ("@kit.ArkUI", "ExceptionPrompt") => Some("@ohos:arkui.advanced.ExceptionPrompt"),
        ("@kit.ArkUI", "ExpandMode") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "ExpandedMenuOptions") => Some("@ohos:arkui.advanced.SelectionMenu"),
        ("@kit.ArkUI", "ExpandedRegionLayoutOptions") => {
            Some("@ohos:arkui.advanced.FoldSplitContainer")
        }
        ("@kit.ArkUI", "ExtraRegionPosition") => Some("@ohos:arkui.advanced.FoldSplitContainer"),
        ("@kit.ArkUI", "Filter") => Some("@ohos:arkui.advanced.Filter"),
        ("@kit.ArkUI", "FilterParams") => Some("@ohos:arkui.advanced.Filter"),
        ("@kit.ArkUI", "FilterResult") => Some("@ohos:arkui.advanced.Filter"),
        ("@kit.ArkUI", "FilterType") => Some("@ohos:arkui.advanced.Filter"),
        ("@kit.ArkUI", "FoldSplitContainer") => Some("@ohos:arkui.advanced.FoldSplitContainer"),
        ("@kit.ArkUI", "FoldedRegionLayoutOptions") => {
            Some("@ohos:arkui.advanced.FoldSplitContainer")
        }
        ("@kit.ArkUI", "Font") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "FormMenuItemStyle") => Some("@ohos:arkui.advanced.FormMenu"),
        ("@kit.ArkUI", "Frame") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "FrameCallback") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "FrameNode") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "FullScreenLaunchComponent") => {
            Some("@ohos:arkui.advanced.FullScreenLaunchComponent")
        }
        ("@kit.ArkUI", "GaugeModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "GestureActionPhase") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "GestureListenerCallback") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "GestureListenerType") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "GestureObserverConfigs") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "GestureTriggerInfo") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "GradientAlpha") => Some("@ohos:atomicservice.AtomicServiceNavigation"),
        ("@kit.ArkUI", "GradientBackground") => Some("@ohos:atomicservice.AtomicServiceNavigation"),
        ("@kit.ArkUI", "GridColModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "GridItemModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "GridModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "GridObjectSortComponent") => {
            Some("@ohos:arkui.advanced.GridObjectSortComponent")
        }
        ("@kit.ArkUI", "GridObjectSortComponentItem") => {
            Some("@ohos:arkui.advanced.GridObjectSortComponent")
        }
        ("@kit.ArkUI", "GridObjectSortComponentOptions") => {
            Some("@ohos:arkui.advanced.GridObjectSortComponent")
        }
        ("@kit.ArkUI", "GridObjectSortComponentType") => {
            Some("@ohos:arkui.advanced.GridObjectSortComponent")
        }
        ("@kit.ArkUI", "GridRowModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "HalfScreenLaunchComponent") => {
            Some("@ohos:atomicservice.HalfScreenLaunchComponent")
        }
        ("@kit.ArkUI", "HoverModeRegionLayoutOptions") => {
            Some("@ohos:arkui.advanced.FoldSplitContainer")
        }
        ("@kit.ArkUI", "HoverModeStatus") => Some("@ohos:arkui.advanced.FoldSplitContainer"),
        ("@kit.ArkUI", "HyperlinkModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "IDataSourcePrefetching") => Some("@ohos:arkui.Prefetcher"),
        ("@kit.ArkUI", "IPrefetcher") => Some("@ohos:arkui.Prefetcher"),
        ("@kit.ArkUI", "IconCommonOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "IconGroupSuffix") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "IconItemOptions") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "IconOptions") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "IconStyle") => Some("@ohos:atomicservice.InterstitialDialogAction"),
        ("@kit.ArkUI", "IconType") => Some("@ohos:arkui.advanced.ComposeListItem"),
        ("@kit.ArkUI", "ImageAnimatorModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ImageModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ImageSpanModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ImmersiveMode") => Some("@ohos:promptAction"),
        ("@kit.ArkUI", "InputEventType") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "InputFilterParams") => Some("@ohos:atomicservice.AtomicServiceSearch"),
        ("@kit.ArkUI", "InterstitialDialogAction") => {
            Some("@ohos:atomicservice.InterstitialDialogAction")
        }
        ("@kit.ArkUI", "ItemRestriction") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "ItemState") => Some("@ohos:arkui.advanced.ToolBar"),
        ("@kit.ArkUI", "KeyboardAvoidMode") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "LabelMarginOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "LabelOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "LayeredDrawableDescriptor") => Some("@ohos:arkui.drawableDescriptor"),
        ("@kit.ArkUI", "LayoutConstraint") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "LengthMetrics") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "LengthMetricsUnit") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "LengthUnit") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "LevelMode") => Some("@ohos:promptAction"),
        ("@kit.ArkUI", "LevelOrder") => Some("@ohos:promptAction"),
        ("@kit.ArkUI", "LineModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ListItemGroupModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ListItemModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ListModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "LoadingDialog") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "LoadingDialogV2") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "LoadingProgressModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "LocaleResponse") => Some("@system:configuration"),
        ("@kit.ArkUI", "MarginType") => Some("@ohos:arkui.advanced.ExceptionPrompt"),
        ("@kit.ArkUI", "MarqueeDynamicSyncScene") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "MarqueeDynamicSyncSceneType") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "MarqueeModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Matrix4") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "MeasureOptions") => Some("@ohos:measure"),
        ("@kit.ArkUI", "MeasureText") => Some("@ohos:measure"),
        ("@kit.ArkUI", "MeasureUtils") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "MediaQuery") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "MediaQueryEvent") => Some("@system:mediaquery"),
        ("@kit.ArkUI", "MediaQueryList") => Some("@system:mediaquery"),
        ("@kit.ArkUI", "MenuAlignParams") => Some("@ohos:atomicservice.AtomicServiceSearch"),
        ("@kit.ArkUI", "MenuItemModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "MenuModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "MixMode") => Some("@ohos:atomicservice.AtomicServiceNavigation"),
        ("@kit.ArkUI", "MultiCapsuleSegmentButtonV2") => {
            Some("@ohos:arkui.advanced.SegmentButtonV2")
        }
        ("@kit.ArkUI", "MultiNavPathStack") => Some("@ohos:arkui.advanced.MultiNavigation"),
        ("@kit.ArkUI", "MultiNavigation") => Some("@ohos:arkui.advanced.MultiNavigation"),
        ("@kit.ArkUI", "MutableBinding") => Some("@ohos:arkui.StateManagement"),
        ("@kit.ArkUI", "NavDestinationBuilder") => {
            Some("@ohos:atomicservice.AtomicServiceNavigation")
        }
        ("@kit.ArkUI", "NavDestinationModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "NavPushPathHelper") => Some("@ohos:atomicservice.NavPushPathHelper"),
        ("@kit.ArkUI", "NavRouterModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "NavigationModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "NavigatorModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "NodeAdapter") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "NodeContent") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "NodeController") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "NodeIdentity") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "NodeParam") => Some("@ohos:arkui.advanced.TreeView"),
        ("@kit.ArkUI", "NodeRenderState") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "NodeRenderStateChangeCallback") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "NodeRenderType") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "Offset") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "OnContentWillChangeCallback") => {
            Some("@ohos:atomicservice.AtomicServiceTabs")
        }
        ("@kit.ArkUI", "OnErrorReceiveEvent") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnHoverStatusChangeHandler") => {
            Some("@ohos:arkui.advanced.FoldSplitContainer")
        }
        ("@kit.ArkUI", "OnHttpErrorReceiveEvent") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnLoadInterceptCallback") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnLoadInterceptEvent") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnMessageEvent") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnPageBeginEvent") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnPageEndEvent") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnProgressChangeEvent") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "OnSelectedIndexChange") => Some("@ohos:arkui.advanced.SegmentButtonV2"),
        ("@kit.ArkUI", "OnSelectedIndexesChange") => Some("@ohos:arkui.advanced.SegmentButtonV2"),
        ("@kit.ArkUI", "OperateButton") => Some("@ohos:arkui.advanced.ComposeListItem"),
        ("@kit.ArkUI", "OperateCheck") => Some("@ohos:arkui.advanced.ComposeListItem"),
        ("@kit.ArkUI", "OperateIcon") => Some("@ohos:arkui.advanced.ComposeListItem"),
        ("@kit.ArkUI", "OperateItem") => Some("@ohos:arkui.advanced.ComposeListItem"),
        ("@kit.ArkUI", "OperationOption") => Some("@ohos:arkui.advanced.SubHeader"),
        ("@kit.ArkUI", "OperationParams") => Some("@ohos:atomicservice.AtomicServiceSearch"),
        ("@kit.ArkUI", "OperationType") => Some("@ohos:arkui.advanced.SubHeader"),
        ("@kit.ArkUI", "OverlayManager") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "OverlayManagerOptions") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "PageInfo") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "PanelModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ParticleModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "PathModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "PathShape") => Some("@ohos:arkui.shape"),
        ("@kit.ArkUI", "PatternLockModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "PersistenceV2") => Some("@ohos:arkui.StateManagement"),
        ("@kit.ArkUI", "PiPWindow") => Some("@ohos:PiPWindow"),
        ("@kit.ArkUI", "Pivot") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "PixelMapDrawableDescriptor") => Some("@ohos:arkui.drawableDescriptor"),
        ("@kit.ArkUI", "PluginComponentTemplate") => Some("@ohos:pluginComponent"),
        ("@kit.ArkUI", "PolygonModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "PolylineModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "PopoverDialog") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "PopoverDialogV2") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "PopoverDialogV2OnVisibleChange") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "PopoverDialogV2Options") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "PopoverOptions") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "Popup") => Some("@ohos:arkui.advanced.Popup"),
        ("@kit.ArkUI", "PopupButtonOptions") => Some("@ohos:arkui.advanced.Popup"),
        ("@kit.ArkUI", "PopupIconOptions") => Some("@ohos:arkui.advanced.Popup"),
        ("@kit.ArkUI", "PopupOptions") => Some("@ohos:arkui.advanced.Popup"),
        ("@kit.ArkUI", "PopupTextOptions") => Some("@ohos:arkui.advanced.Popup"),
        ("@kit.ArkUI", "Position") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "PrefixIconOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "PresetSplitRatio") => Some("@ohos:arkui.advanced.FoldSplitContainer"),
        ("@kit.ArkUI", "ProgressButton") => Some("@ohos:arkui.advanced.ProgressButton"),
        ("@kit.ArkUI", "ProgressButtonV2") => Some("@ohos:arkui.advanced.ProgressButtonV2"),
        ("@kit.ArkUI", "ProgressButtonV2Color") => Some("@ohos:arkui.advanced.ProgressButtonV2"),
        ("@kit.ArkUI", "ProgressButtonV2ColorOptions") => {
            Some("@ohos:arkui.advanced.ProgressButtonV2")
        }
        ("@kit.ArkUI", "ProgressModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Prompt") => Some("@system:prompt"),
        ("@kit.ArkUI", "PromptAction") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "PromptOptions") => Some("@ohos:arkui.advanced.ExceptionPrompt"),
        ("@kit.ArkUI", "QRCodeModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "RadioModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "RatingModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Rect") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "RectModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "RectShape") => Some("@ohos:arkui.shape"),
        ("@kit.ArkUI", "RefreshModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "RenderNode") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "RenderOptions") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "RequestFullWindowOptions") => Some("@system:app"),
        ("@kit.ArkUI", "RichEditorModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Rotation") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "RoundRect") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "Router") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "RouterOptions") => Some("@system:router"),
        ("@kit.ArkUI", "RouterState") => Some("@system:router"),
        ("@kit.ArkUI", "RowModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "RowSplitModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Scale") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "ScreenOnVisibleOptions") => Some("@system:app"),
        ("@kit.ArkUI", "ScrollModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SearchButtonParams") => Some("@ohos:atomicservice.AtomicServiceSearch"),
        ("@kit.ArkUI", "SearchModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SearchParams") => Some("@ohos:atomicservice.AtomicServiceSearch"),
        ("@kit.ArkUI", "SegmentButton") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonIconItem") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonIconTextItem") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonItemArray") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonItemOptions") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonItemOptionsArray") => {
            Some("@ohos:arkui.advanced.SegmentButton")
        }
        ("@kit.ArkUI", "SegmentButtonItemOptionsConstructorOptions") => {
            Some("@ohos:arkui.advanced.SegmentButton")
        }
        ("@kit.ArkUI", "SegmentButtonItemTuple") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonOptions") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonTextItem") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "SegmentButtonV2Item") => Some("@ohos:arkui.advanced.SegmentButtonV2"),
        ("@kit.ArkUI", "SegmentButtonV2ItemOptions") => {
            Some("@ohos:arkui.advanced.SegmentButtonV2")
        }
        ("@kit.ArkUI", "SegmentButtonV2Items") => Some("@ohos:arkui.advanced.SegmentButtonV2"),
        ("@kit.ArkUI", "SelectDialog") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "SelectDialogV2") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "SelectModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SelectOptions") => Some("@ohos:arkui.advanced.SubHeader"),
        ("@kit.ArkUI", "SelectParams") => Some("@ohos:atomicservice.AtomicServiceSearch"),
        ("@kit.ArkUI", "SelectTitleBar") => Some("@ohos:arkui.advanced.SelectTitleBar"),
        ("@kit.ArkUI", "SelectTitleBarMenuItem") => Some("@ohos:arkui.advanced.SelectTitleBar"),
        ("@kit.ArkUI", "SelectionMenu") => Some("@ohos:arkui.advanced.SelectionMenu"),
        ("@kit.ArkUI", "SelectionMenuOptions") => Some("@ohos:arkui.advanced.SelectionMenu"),
        ("@kit.ArkUI", "ShapeClip") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "ShapeMask") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "ShapeModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ShowActionMenuOptions") => Some("@system:prompt"),
        ("@kit.ArkUI", "ShowDialogOptions") => Some("@system:prompt"),
        ("@kit.ArkUI", "ShowDialogSuccessResponse") => Some("@system:prompt"),
        ("@kit.ArkUI", "ShowToastOptions") => Some("@system:prompt"),
        ("@kit.ArkUI", "SideBarContainerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SideBarOptions") => Some("@ohos:atomicservice.AtomicServiceNavigation"),
        ("@kit.ArkUI", "SimpleAnimatorOptions") => Some("@ohos:animator"),
        ("@kit.ArkUI", "Size") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "SliderModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SpanModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SplitLayout") => Some("@ohos:arkui.advanced.SplitLayout"),
        ("@kit.ArkUI", "SplitPolicy") => Some("@ohos:arkui.advanced.MultiNavigation"),
        ("@kit.ArkUI", "StackModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "StepperItemModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "StepperModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SubHeader") => Some("@ohos:arkui.advanced.SubHeader"),
        ("@kit.ArkUI", "SubHeaderV2") => Some("@ohos:arkui.advanced.SubHeaderV2"),
        ("@kit.ArkUI", "SubHeaderV2IconType") => Some("@ohos:arkui.advanced.SubHeaderV2"),
        ("@kit.ArkUI", "SubHeaderV2OperationItem") => Some("@ohos:arkui.advanced.SubHeaderV2"),
        ("@kit.ArkUI", "SubHeaderV2OperationItemType") => Some("@ohos:arkui.advanced.SubHeaderV2"),
        ("@kit.ArkUI", "SubHeaderV2OperationType") => Some("@ohos:arkui.advanced.SubHeaderV2"),
        ("@kit.ArkUI", "SubHeaderV2Select") => Some("@ohos:arkui.advanced.SubHeaderV2"),
        ("@kit.ArkUI", "SubHeaderV2Title") => Some("@ohos:arkui.advanced.SubHeaderV2"),
        ("@kit.ArkUI", "SuffixIconOptions") => Some("@ohos:arkui.advanced.Chip"),
        ("@kit.ArkUI", "SuffixImageIconOptions") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "SwipeRefresher") => Some("@ohos:arkui.advanced.SwipeRefresher"),
        ("@kit.ArkUI", "SwiperDynamicSyncScene") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "SwiperDynamicSyncSceneType") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "SwiperModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SymbolGlyphModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SymbolItemOptions") => Some("@ohos:arkui.advanced.ChipGroup"),
        ("@kit.ArkUI", "SymbolOptions") => Some("@ohos:arkui.advanced.SubHeader"),
        ("@kit.ArkUI", "SymbolSpanModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "SystemMediaQuery") => Some("@system:mediaquery"),
        ("@kit.ArkUI", "SystemRouter") => Some("@system:router"),
        ("@kit.ArkUI", "TabBarOptions") => Some("@ohos:atomicservice.AtomicServiceTabs"),
        ("@kit.ArkUI", "TabBarPosition") => Some("@ohos:atomicservice.AtomicServiceTabs"),
        ("@kit.ArkUI", "TabContentBuilder") => Some("@ohos:atomicservice.AtomicServiceTabs"),
        ("@kit.ArkUI", "TabSegmentButtonConstructionOptions") => {
            Some("@ohos:arkui.advanced.SegmentButton")
        }
        ("@kit.ArkUI", "TabSegmentButtonOptions") => Some("@ohos:arkui.advanced.SegmentButton"),
        ("@kit.ArkUI", "TabSegmentButtonV2") => Some("@ohos:arkui.advanced.SegmentButtonV2"),
        ("@kit.ArkUI", "TabTitleBar") => Some("@ohos:arkui.advanced.TabTitleBar"),
        ("@kit.ArkUI", "TabTitleBarMenuItem") => Some("@ohos:arkui.advanced.TabTitleBar"),
        ("@kit.ArkUI", "TabTitleBarTabItem") => Some("@ohos:arkui.advanced.TabTitleBar"),
        ("@kit.ArkUI", "TabsModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "TargetInfo") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "TextAreaModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "TextClockModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "TextInputModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "TextMenuController") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "TextModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "TextPickerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "TextTimerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "Theme") => Some("@ohos:arkui.theme"),
        ("@kit.ArkUI", "ThemeControl") => Some("@ohos:arkui.theme"),
        ("@kit.ArkUI", "TimePickerModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "TipsDialog") => Some("@ohos:arkui.advanced.Dialog"),
        ("@kit.ArkUI", "TipsDialogV2") => Some("@ohos:arkui.advanced.DialogV2"),
        ("@kit.ArkUI", "TitleBarType") => Some("@ohos:atomicservice.AtomicServiceNavigation"),
        ("@kit.ArkUI", "TitleOptions") => Some("@ohos:atomicservice.AtomicServiceNavigation"),
        ("@kit.ArkUI", "TitlePosition") => Some("@ohos:atomicservice.InterstitialDialogAction"),
        ("@kit.ArkUI", "ToggleModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "ToolBar") => Some("@ohos:arkui.advanced.ToolBar"),
        ("@kit.ArkUI", "ToolBarModifier") => Some("@ohos:arkui.advanced.ToolBar"),
        ("@kit.ArkUI", "ToolBarOption") => Some("@ohos:arkui.advanced.ToolBar"),
        ("@kit.ArkUI", "ToolBarOptions") => Some("@ohos:arkui.advanced.ToolBar"),
        ("@kit.ArkUI", "ToolBarV2") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2Item") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemAction") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemIconType") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemImage") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemImageOptions") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemOptions") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemState") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemText") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2ItemTextOptions") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2Modifier") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2SymbolGlyph") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "ToolBarV2SymbolGlyphOptions") => Some("@ohos:arkui.advanced.ToolBarV2"),
        ("@kit.ArkUI", "Translation") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "TreeController") => Some("@ohos:arkui.advanced.TreeView"),
        ("@kit.ArkUI", "TreeListenType") => Some("@ohos:arkui.advanced.TreeView"),
        ("@kit.ArkUI", "TreeListener") => Some("@ohos:arkui.advanced.TreeView"),
        ("@kit.ArkUI", "TreeListenerManager") => Some("@ohos:arkui.advanced.TreeView"),
        ("@kit.ArkUI", "TreeView") => Some("@ohos:arkui.advanced.TreeView"),
        ("@kit.ArkUI", "Type") => Some("@ohos:arkui.StateManagement"),
        ("@kit.ArkUI", "UIContext") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "UIInspector") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "UIObserver") => Some("@ohos:arkui.UIContext"),
        ("@kit.ArkUI", "UIState") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "UIUtils") => Some("@ohos:arkui.StateManagement"),
        ("@kit.ArkUI", "VideoModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "WaterFlowModifier") => Some("@ohos:arkui.modifier"),
        ("@kit.ArkUI", "WebHeader") => Some("@ohos:atomicservice.AtomicServiceWeb"),
        ("@kit.ArkUI", "XComponentNode") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "borderRadiuses") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "borderStyles") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "componentSnapshot") => Some("@ohos:arkui.componentSnapshot"),
        ("@kit.ArkUI", "componentUtils") => Some("@ohos:arkui.componentUtils"),
        ("@kit.ArkUI", "curves") => Some("@ohos:curves"),
        ("@kit.ArkUI", "display") => Some("@ohos:display"),
        ("@kit.ArkUI", "dragController") => Some("@ohos:arkui.dragController"),
        ("@kit.ArkUI", "edgeColors") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "edgeWidths") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "floatingBall") => Some("@ohos:window.floatingBall"),
        ("@kit.ArkUI", "font") => Some("@ohos:font"),
        ("@kit.ArkUI", "inspector") => Some("@ohos:arkui.inspector"),
        ("@kit.ArkUI", "matrix4") => Some("@ohos:matrix4"),
        ("@kit.ArkUI", "mediaquery") => Some("@ohos:mediaquery"),
        ("@kit.ArkUI", "pluginComponentManager") => Some("@ohos:pluginComponent"),
        ("@kit.ArkUI", "prompt") => Some("@ohos:prompt"),
        ("@kit.ArkUI", "promptAction") => Some("@ohos:promptAction"),
        ("@kit.ArkUI", "router") => Some("@ohos:router"),
        ("@kit.ArkUI", "screenshot") => Some("@ohos:screenshot"),
        ("@kit.ArkUI", "typeNode") => Some("@ohos:arkui.node"),
        ("@kit.ArkUI", "uiAppearance") => Some("@ohos:uiAppearance"),
        ("@kit.ArkUI", "uiExtension") => Some("@ohos:arkui.uiExtension"),
        ("@kit.ArkUI", "uiObserver") => Some("@ohos:arkui.observer"),
        ("@kit.ArkUI", "window") => Some("@ohos:window"),

        // ── @kit.ArkWeb ─────────────────────────────────────────────────────────
        ("@kit.ArkWeb", "ConnectionInfo") => Some("@ohos:web.WebNativeMessagingExtensionAbility"),
        ("@kit.ArkWeb", "WebNativeMessagingExtensionAbility") => {
            Some("@ohos:web.WebNativeMessagingExtensionAbility")
        }
        ("@kit.ArkWeb", "WebNetErrorList") => Some("@ohos:web.netErrorList"),
        ("@kit.ArkWeb", "webNativeMessagingExtensionManager") => {
            Some("@ohos:web.webNativeMessagingExtensionManager")
        }
        ("@kit.ArkWeb", "webview") => Some("@ohos:web.webview"),

        // ── @kit.AssetStoreKit ──────────────────────────────────────────────────
        ("@kit.AssetStoreKit", _) => Some("@ohos:security.asset"),

        // ── @kit.AudioKit ───────────────────────────────────────────────────────
        ("@kit.AudioKit", "AVVolumePanel") => Some("@ohos:multimedia.avVolumePanel"),
        ("@kit.AudioKit", "AVVolumePanelParameter") => Some("@ohos:multimedia.avVolumePanel"),
        ("@kit.AudioKit", "audio") => Some("@ohos:multimedia.audio"),
        ("@kit.AudioKit", "audioHaptic") => Some("@ohos:multimedia.audioHaptic"),

        // ── @kit.BackgroundTasksKit ─────────────────────────────────────────────
        ("@kit.BackgroundTasksKit", "WorkSchedulerExtensionAbility") => {
            Some("@ohos:WorkSchedulerExtensionAbility")
        }
        ("@kit.BackgroundTasksKit", "WorkSchedulerExtensionContext") => {
            Some("@ohos:WorkSchedulerExtensionAbility")
        }
        ("@kit.BackgroundTasksKit", "backgroundProcessManager") => {
            Some("@ohos:resourceschedule.backgroundProcessManager")
        }
        ("@kit.BackgroundTasksKit", "backgroundTaskManager") => {
            Some("@ohos:resourceschedule.backgroundTaskManager")
        }
        ("@kit.BackgroundTasksKit", "bundleState") => Some("@ohos:bundleState"),
        ("@kit.BackgroundTasksKit", "deviceStandby") => {
            Some("@ohos:resourceschedule.deviceStandby")
        }
        ("@kit.BackgroundTasksKit", "reminderAgent") => Some("@ohos:reminderAgent"),
        ("@kit.BackgroundTasksKit", "reminderAgentManager") => Some("@ohos:reminderAgentManager"),
        ("@kit.BackgroundTasksKit", "usageStatistics") => {
            Some("@ohos:resourceschedule.usageStatistics")
        }
        ("@kit.BackgroundTasksKit", "workScheduler") => {
            Some("@ohos:resourceschedule.workScheduler")
        }

        // ── @kit.BasicServicesKit ───────────────────────────────────────────────
        ("@kit.BasicServicesKit", "AsyncCallback") => Some("@ohos:base"),
        ("@kit.BasicServicesKit", "Battery") => Some("@system:battery"),
        ("@kit.BasicServicesKit", "BatteryResponse") => Some("@system:battery"),
        ("@kit.BasicServicesKit", "Brightness") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "BrightnessModeResponse") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "BrightnessResponse") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "BusinessError") => Some("@ohos:base"),
        ("@kit.BasicServicesKit", "Callback") => Some("@ohos:base"),
        ("@kit.BasicServicesKit", "Device") => Some("@system:device"),
        ("@kit.BasicServicesKit", "DeviceResponse") => Some("@system:device"),
        ("@kit.BasicServicesKit", "DownloadRequestOptions") => Some("@system:request"),
        ("@kit.BasicServicesKit", "DownloadResponse") => Some("@system:request"),
        ("@kit.BasicServicesKit", "ErrorCallback") => Some("@ohos:base"),
        ("@kit.BasicServicesKit", "GetBrightnessModeOptions") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "GetBrightnessOptions") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "GetDeviceOptions") => Some("@system:device"),
        ("@kit.BasicServicesKit", "GetStatusOptions") => Some("@system:battery"),
        ("@kit.BasicServicesKit", "OnDownloadCompleteOptions") => Some("@system:request"),
        ("@kit.BasicServicesKit", "OnDownloadCompleteResponse") => Some("@system:request"),
        ("@kit.BasicServicesKit", "PrintExtensionAbility") => {
            Some("@ohos:app.ability.PrintExtensionAbility")
        }
        ("@kit.BasicServicesKit", "Request") => Some("@system:request"),
        ("@kit.BasicServicesKit", "RequestData") => Some("@system:request"),
        ("@kit.BasicServicesKit", "RequestFile") => Some("@system:request"),
        ("@kit.BasicServicesKit", "SetBrightnessModeOptions") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "SetBrightnessOptions") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "SetKeepScreenOnOptions") => Some("@system:brightness"),
        ("@kit.BasicServicesKit", "UploadRequestOptions") => Some("@system:request"),
        ("@kit.BasicServicesKit", "UploadResponse") => Some("@system:request"),
        ("@kit.BasicServicesKit", "appAccount") => Some("@ohos:account.appAccount"),
        ("@kit.BasicServicesKit", "batteryInfo") => Some("@ohos:batteryInfo"),
        ("@kit.BasicServicesKit", "cacheDownload") => Some("@ohos:request.cacheDownload"),
        ("@kit.BasicServicesKit", "commonEventManager") => Some("@ohos:commonEventManager"),
        ("@kit.BasicServicesKit", "customConfig") => Some("@ohos:customization.customConfig"),
        ("@kit.BasicServicesKit", "deviceAttest") => Some("@ohos:deviceAttest"),
        ("@kit.BasicServicesKit", "deviceInfo") => Some("@ohos:deviceInfo"),
        ("@kit.BasicServicesKit", "distributedAccount") => Some("@ohos:account.distributedAccount"),
        ("@kit.BasicServicesKit", "emitter") => Some("@ohos:events.emitter"),
        ("@kit.BasicServicesKit", "osAccount") => Some("@ohos:account.osAccount"),
        ("@kit.BasicServicesKit", "pasteboard") => Some("@ohos:pasteboard"),
        ("@kit.BasicServicesKit", "power") => Some("@ohos:power"),
        ("@kit.BasicServicesKit", "print") => Some("@ohos:print"),
        ("@kit.BasicServicesKit", "request") => Some("@ohos:request"),
        ("@kit.BasicServicesKit", "runningLock") => Some("@ohos:runningLock"),
        ("@kit.BasicServicesKit", "scan") => Some("@ohos:scan"),
        ("@kit.BasicServicesKit", "screenLock") => Some("@ohos:screenLock"),
        ("@kit.BasicServicesKit", "serialManager") => Some("@ohos:usbManager.serial"),
        ("@kit.BasicServicesKit", "settings") => Some("@ohos:settings"),
        ("@kit.BasicServicesKit", "systemDateTime") => Some("@ohos:systemDateTime"),
        ("@kit.BasicServicesKit", "systemLoad") => Some("@ohos:resourceschedule.systemload"),
        ("@kit.BasicServicesKit", "systemTime") => Some("@ohos:systemTime"),
        ("@kit.BasicServicesKit", "thermal") => Some("@ohos:thermal"),
        ("@kit.BasicServicesKit", "usb") => Some("@ohos:usb"),
        ("@kit.BasicServicesKit", "usbManager") => Some("@ohos:usbManager"),
        ("@kit.BasicServicesKit", "wallpaper") => Some("@ohos:wallpaper"),
        ("@kit.BasicServicesKit", "zlib") => Some("@ohos:zlib"),

        // ── @kit.CalendarKit ────────────────────────────────────────────────────
        ("@kit.CalendarKit", _) => Some("@ohos:calendarManager"),

        // ── @kit.CameraKit ──────────────────────────────────────────────────────
        ("@kit.CameraKit", "camera") => Some("@ohos:multimedia.camera"),
        ("@kit.CameraKit", "cameraPicker") => Some("@ohos:multimedia.cameraPicker"),

        // ── @kit.ConnectivityKit ────────────────────────────────────────────────
        ("@kit.ConnectivityKit", "BLEFoundResponse") => Some("@system:bluetooth"),
        ("@kit.ConnectivityKit", "Bluetooth") => Some("@system:bluetooth"),
        ("@kit.ConnectivityKit", "BluetoothDevice") => Some("@system:bluetooth"),
        ("@kit.ConnectivityKit", "StartBLEScanOptions") => Some("@system:bluetooth"),
        ("@kit.ConnectivityKit", "StopBLEScanOptions") => Some("@system:bluetooth"),
        ("@kit.ConnectivityKit", "SubscribeBLEFoundOptions") => Some("@system:bluetooth"),
        ("@kit.ConnectivityKit", "a2dp") => Some("@ohos:bluetooth.a2dp"),
        ("@kit.ConnectivityKit", "access") => Some("@ohos:bluetooth.access"),
        ("@kit.ConnectivityKit", "baseProfile") => Some("@ohos:bluetooth.baseProfile"),
        ("@kit.ConnectivityKit", "ble") => Some("@ohos:bluetooth.ble"),
        ("@kit.ConnectivityKit", "bluetooth") => Some("@ohos:bluetooth"),
        ("@kit.ConnectivityKit", "bluetoothManager") => Some("@ohos:bluetoothManager"),
        ("@kit.ConnectivityKit", "cardEmulation") => Some("@ohos:nfc.cardEmulation"),
        ("@kit.ConnectivityKit", "common") => Some("@ohos:bluetooth.common"),
        ("@kit.ConnectivityKit", "connectedTag") => Some("@ohos:connectedTag"),
        ("@kit.ConnectivityKit", "connection") => Some("@ohos:bluetooth.connection"),
        ("@kit.ConnectivityKit", "constant") => Some("@ohos:bluetooth.constant"),
        ("@kit.ConnectivityKit", "hfp") => Some("@ohos:bluetooth.hfp"),
        ("@kit.ConnectivityKit", "hid") => Some("@ohos:bluetooth.hid"),
        ("@kit.ConnectivityKit", "map") => Some("@ohos:bluetooth.map"),
        ("@kit.ConnectivityKit", "nfcController") => Some("@ohos:nfc.controller"),
        ("@kit.ConnectivityKit", "omapi") => Some("@ohos:secureElement"),
        ("@kit.ConnectivityKit", "opp") => Some("@ohos:bluetooth.opp"),
        ("@kit.ConnectivityKit", "pan") => Some("@ohos:bluetooth.pan"),
        ("@kit.ConnectivityKit", "pbap") => Some("@ohos:bluetooth.pbap"),
        ("@kit.ConnectivityKit", "socket") => Some("@ohos:bluetooth.socket"),
        ("@kit.ConnectivityKit", "tag") => Some("@ohos:nfc.tag"),
        ("@kit.ConnectivityKit", "wearDetection") => Some("@ohos:bluetooth.wearDetection"),
        ("@kit.ConnectivityKit", "wifi") => Some("@ohos:wifi"),
        ("@kit.ConnectivityKit", "wifiManager") => Some("@ohos:wifiManager"),
        ("@kit.ConnectivityKit", "wifiManagerExt") => Some("@ohos:wifiManagerExt"),
        ("@kit.ConnectivityKit", "wifiext") => Some("@ohos:wifiext"),

        // ── @kit.ContactsKit ────────────────────────────────────────────────────
        ("@kit.ContactsKit", _) => Some("@ohos:contact"),

        // ── @kit.CoreFileKit ────────────────────────────────────────────────────
        ("@kit.CoreFileKit", "BackupExtensionAbility") => {
            Some("@ohos:application.BackupExtensionAbility")
        }
        ("@kit.CoreFileKit", "BackupExtensionContext") => Some("@ohos:file.BackupExtensionContext"),
        ("@kit.CoreFileKit", "BundleVersion") => Some("@ohos:application.BackupExtensionAbility"),
        ("@kit.CoreFileKit", "ConflictFiles") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "DfsListeners") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "Environment") => Some("@ohos:file.environment"),
        ("@kit.CoreFileKit", "Filter") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "ListFileOptions") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "Options") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "ReadOptions") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "ReadTextOptions") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "ReaderIteratorResult") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "TaskSignal") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "WatchEvent") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "WatchEventListener") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "Watcher") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "WriteOptions") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "cloudSync") => Some("@ohos:file.cloudSync"),
        ("@kit.CoreFileKit", "cloudSyncManager") => Some("@ohos:file.cloudSyncManager"),
        ("@kit.CoreFileKit", "fileAccess") => Some("@ohos:file.fileAccess"),
        ("@kit.CoreFileKit", "fileIo") => Some("@ohos:file.fs"),
        ("@kit.CoreFileKit", "fileShare") => Some("@ohos:fileshare"),
        ("@kit.CoreFileKit", "fileUri") => Some("@ohos:file.fileuri"),
        ("@kit.CoreFileKit", "hash") => Some("@ohos:file.hash"),
        ("@kit.CoreFileKit", "keyManager") => Some("@ohos:file.keyManager"),
        ("@kit.CoreFileKit", "picker") => Some("@ohos:file.picker"),
        ("@kit.CoreFileKit", "securityLabel") => Some("@ohos:file.securityLabel"),
        ("@kit.CoreFileKit", "statfs") => Some("@ohos:file.statvfs"),
        ("@kit.CoreFileKit", "storageStatistics") => Some("@ohos:file.storageStatistics"),

        // ── @kit.CryptoArchitectureKit ──────────────────────────────────────────
        ("@kit.CryptoArchitectureKit", "Cipher") => Some("@system:cipher"),
        ("@kit.CryptoArchitectureKit", "CipherAesOptions") => Some("@system:cipher"),
        ("@kit.CryptoArchitectureKit", "CipherResponse") => Some("@system:cipher"),
        ("@kit.CryptoArchitectureKit", "CipherRsaOptions") => Some("@system:cipher"),
        ("@kit.CryptoArchitectureKit", "cryptoFramework") => Some("@ohos:security.cryptoFramework"),

        // ── @kit.DataLossPreventionKit ──────────────────────────────────────────
        ("@kit.DataLossPreventionKit", _) => Some("@ohos:dlpPermission"),

        // ── @kit.DataProtectionKit ──────────────────────────────────────────────
        ("@kit.DataProtectionKit", "dlpPermission") => Some("@ohos:dlpPermission"),
        ("@kit.DataProtectionKit", "identifySensitiveContent") => {
            Some("@ohos:security.identifySensitiveContent")
        }

        // ── @kit.DeviceCertificateKit ───────────────────────────────────────────
        ("@kit.DeviceCertificateKit", "cert") => Some("@ohos:security.cert"),
        ("@kit.DeviceCertificateKit", "certificateManager") => Some("@ohos:security.certManager"),
        ("@kit.DeviceCertificateKit", "certificateManagerDialog") => {
            Some("@ohos:security.certManagerDialog")
        }

        // ── @kit.DistributedServiceKit ──────────────────────────────────────────
        ("@kit.DistributedServiceKit", "DistributedExtensionAbility") => {
            Some("@ohos:application.DistributedExtensionAbility")
        }
        ("@kit.DistributedServiceKit", "DistributedExtensionContext") => {
            Some("@ohos:application.DistributedExtensionContext")
        }
        ("@kit.DistributedServiceKit", "abilityConnectionManager") => {
            Some("@ohos:distributedsched.abilityConnectionManager")
        }
        ("@kit.DistributedServiceKit", "deviceManager") => {
            Some("@ohos:distributedHardware.deviceManager")
        }
        ("@kit.DistributedServiceKit", "distributedDeviceManager") => {
            Some("@ohos:distributedDeviceManager")
        }
        ("@kit.DistributedServiceKit", "linkEnhance") => Some("@ohos:distributedsched.linkEnhance"),
        ("@kit.DistributedServiceKit", "proxyChannelManager") => {
            Some("@ohos:distributedsched.proxyChannelManager")
        }

        // ── @kit.DriverDevelopmentKit ───────────────────────────────────────────
        ("@kit.DriverDevelopmentKit", "DriverExtensionAbility") => {
            Some("@ohos:app.ability.DriverExtensionAbility")
        }
        ("@kit.DriverDevelopmentKit", "DriverExtensionContext") => {
            Some("@ohos:app.ability.DriverExtensionAbility")
        }
        ("@kit.DriverDevelopmentKit", "deviceManager") => Some("@ohos:driver.deviceManager"),

        // ── @kit.DrmKit ─────────────────────────────────────────────────────────
        ("@kit.DrmKit", _) => Some("@ohos:multimedia.drm"),

        // ── @kit.FormKit ────────────────────────────────────────────────────────
        ("@kit.FormKit", "FormEditExtensionAbility") => {
            Some("@ohos:app.form.FormEditExtensionAbility")
        }
        ("@kit.FormKit", "FormExtensionAbility") => Some("@ohos:app.form.FormExtensionAbility"),
        ("@kit.FormKit", "LiveFormExtensionAbility") => {
            Some("@ohos:app.form.LiveFormExtensionAbility")
        }
        ("@kit.FormKit", "LiveFormInfo") => Some("@ohos:app.form.LiveFormExtensionAbility"),
        ("@kit.FormKit", "formBindingData") => Some("@ohos:app.form.formBindingData"),
        ("@kit.FormKit", "formError") => Some("@ohos:application.formError"),
        ("@kit.FormKit", "formInfo") => Some("@ohos:app.form.formInfo"),
        ("@kit.FormKit", "formProvider") => Some("@ohos:app.form.formProvider"),

        // ── @kit.IMEKit ─────────────────────────────────────────────────────────
        ("@kit.IMEKit", "InputMethodExtensionAbility") => Some("@ohos:InputMethodExtensionAbility"),
        ("@kit.IMEKit", "InputMethodExtensionContext") => Some("@ohos:InputMethodExtensionContext"),
        ("@kit.IMEKit", "InputMethodListDialog") => Some("@ohos:inputMethodList"),
        ("@kit.IMEKit", "InputMethodSubtype") => Some("@ohos:InputMethodSubtype"),
        ("@kit.IMEKit", "PanelFlag") => Some("@ohos:inputMethod.Panel"),
        ("@kit.IMEKit", "PanelInfo") => Some("@ohos:inputMethod.Panel"),
        ("@kit.IMEKit", "PanelType") => Some("@ohos:inputMethod.Panel"),
        ("@kit.IMEKit", "Pattern") => Some("@ohos:inputMethodList"),
        ("@kit.IMEKit", "PatternOptions") => Some("@ohos:inputMethodList"),
        ("@kit.IMEKit", "inputMethod") => Some("@ohos:inputMethod"),
        ("@kit.IMEKit", "inputMethodEngine") => Some("@ohos:inputMethodEngine"),

        // ── @kit.IPCKit ─────────────────────────────────────────────────────────
        ("@kit.IPCKit", _) => Some("@ohos:rpc"),

        // ── @kit.ImageKit ───────────────────────────────────────────────────────
        ("@kit.ImageKit", "image") => Some("@ohos:multimedia.image"),
        ("@kit.ImageKit", "sendableImage") => Some("@ohos:multimedia.sendableImage"),
        ("@kit.ImageKit", "videoProcessingEngine") => {
            Some("@ohos:multimedia.videoProcessingEngine")
        }

        // ── @kit.InputKit ───────────────────────────────────────────────────────
        ("@kit.InputKit", "Action") => Some("@ohos:multimodalInput.keyEvent"),
        ("@kit.InputKit", "ActionType") => Some("@ohos:multimodalInput.gestureEvent"),
        ("@kit.InputKit", "Axis") => Some("@ohos:multimodalInput.mouseEvent"),
        ("@kit.InputKit", "AxisValue") => Some("@ohos:multimodalInput.mouseEvent"),
        ("@kit.InputKit", "Button") => Some("@ohos:multimodalInput.mouseEvent"),
        ("@kit.InputKit", "FourFingersSwipe") => Some("@ohos:multimodalInput.gestureEvent"),
        ("@kit.InputKit", "InputEvent") => Some("@ohos:multimodalInput.inputEvent"),
        ("@kit.InputKit", "IntentionCode") => Some("@ohos:multimodalInput.intentionCode"),
        ("@kit.InputKit", "Key") => Some("@ohos:multimodalInput.keyEvent"),
        ("@kit.InputKit", "KeyAction") => Some("@ohos:multimodalInput.touchEvent"),
        ("@kit.InputKit", "KeyCode") => Some("@ohos:multimodalInput.keyCode"),
        ("@kit.InputKit", "KeyEvent") => Some("@ohos:multimodalInput.keyEvent"),
        ("@kit.InputKit", "MouseAction") => Some("@ohos:multimodalInput.mouseEvent"),
        ("@kit.InputKit", "MouseEvent") => Some("@ohos:multimodalInput.mouseEvent"),
        ("@kit.InputKit", "MouseToolType") => Some("@ohos:multimodalInput.mouseEvent"),
        ("@kit.InputKit", "Pinch") => Some("@ohos:multimodalInput.gestureEvent"),
        ("@kit.InputKit", "Rotate") => Some("@ohos:multimodalInput.gestureEvent"),
        ("@kit.InputKit", "SourceType") => Some("@ohos:multimodalInput.touchEvent"),
        ("@kit.InputKit", "ThreeFingersSwipe") => Some("@ohos:multimodalInput.gestureEvent"),
        ("@kit.InputKit", "ThreeFingersTap") => Some("@ohos:multimodalInput.gestureEvent"),
        ("@kit.InputKit", "ToolType") => Some("@ohos:multimodalInput.touchEvent"),
        ("@kit.InputKit", "Touch") => Some("@ohos:multimodalInput.touchEvent"),
        ("@kit.InputKit", "TouchEvent") => Some("@ohos:multimodalInput.touchEvent"),
        ("@kit.InputKit", "infraredEmitter") => Some("@ohos:multimodalInput.infraredEmitter"),
        ("@kit.InputKit", "inputConsumer") => Some("@ohos:multimodalInput.inputConsumer"),
        ("@kit.InputKit", "inputDevice") => Some("@ohos:multimodalInput.inputDevice"),
        ("@kit.InputKit", "inputDeviceCooperate") => {
            Some("@ohos:multimodalInput.inputDeviceCooperate")
        }
        ("@kit.InputKit", "pointer") => Some("@ohos:multimodalInput.pointer"),

        // ── @kit.LocalizationKit ────────────────────────────────────────────────
        ("@kit.LocalizationKit", "i18n") => Some("@ohos:i18n"),
        ("@kit.LocalizationKit", "intl") => Some("@ohos:intl"),
        ("@kit.LocalizationKit", "resourceManager") => Some("@ohos:resourceManager"),
        ("@kit.LocalizationKit", "sendableResourceManager") => {
            Some("@ohos:sendableResourceManager")
        }

        // ── @kit.LocationKit ────────────────────────────────────────────────────
        ("@kit.LocationKit", "FenceExtensionAbility") => {
            Some("@ohos:app.ability.FenceExtensionAbility")
        }
        ("@kit.LocationKit", "FenceExtensionContext") => {
            Some("@ohos:app.ability.FenceExtensionContext")
        }
        ("@kit.LocationKit", "Geolocation") => Some("@system:geolocation"),
        ("@kit.LocationKit", "GeolocationResponse") => Some("@system:geolocation"),
        ("@kit.LocationKit", "GetLocationOption") => Some("@system:geolocation"),
        ("@kit.LocationKit", "GetLocationTypeOption") => Some("@system:geolocation"),
        ("@kit.LocationKit", "GetLocationTypeResponse") => Some("@system:geolocation"),
        ("@kit.LocationKit", "SubscribeLocationOption") => Some("@system:geolocation"),
        ("@kit.LocationKit", "geoLocationManager") => Some("@ohos:geoLocationManager"),
        ("@kit.LocationKit", "geolocation") => Some("@ohos:geolocation"),

        // ── @kit.MDMKit ─────────────────────────────────────────────────────────
        ("@kit.MDMKit", "EnterpriseAdminExtensionAbility") => {
            Some("@ohos:enterprise.EnterpriseAdminExtensionAbility")
        }
        ("@kit.MDMKit", "accountManager") => Some("@ohos:enterprise.accountManager"),
        ("@kit.MDMKit", "adminManager") => Some("@ohos:enterprise.adminManager"),
        ("@kit.MDMKit", "applicationManager") => Some("@ohos:enterprise.applicationManager"),
        ("@kit.MDMKit", "bluetoothManager") => Some("@ohos:enterprise.bluetoothManager"),
        ("@kit.MDMKit", "browser") => Some("@ohos:enterprise.browser"),
        ("@kit.MDMKit", "bundleManager") => Some("@ohos:enterprise.bundleManager"),
        ("@kit.MDMKit", "deviceControl") => Some("@ohos:enterprise.deviceControl"),
        ("@kit.MDMKit", "deviceInfo") => Some("@ohos:enterprise.deviceInfo"),
        ("@kit.MDMKit", "deviceSettings") => Some("@ohos:enterprise.deviceSettings"),
        ("@kit.MDMKit", "locationManager") => Some("@ohos:enterprise.locationManager"),
        ("@kit.MDMKit", "networkManager") => Some("@ohos:enterprise.networkManager"),
        ("@kit.MDMKit", "restrictions") => Some("@ohos:enterprise.restrictions"),
        ("@kit.MDMKit", "securityManager") => Some("@ohos:enterprise.securityManager"),
        ("@kit.MDMKit", "systemManager") => Some("@ohos:enterprise.systemManager"),
        ("@kit.MDMKit", "telephonyManager") => Some("@ohos:enterprise.telephonyManager"),
        ("@kit.MDMKit", "usbManager") => Some("@ohos:enterprise.usbManager"),
        ("@kit.MDMKit", "wifiManager") => Some("@ohos:enterprise.wifiManager"),

        // ── @kit.MechanicKit ────────────────────────────────────────────────────
        ("@kit.MechanicKit", _) => Some("@ohos:distributedHardware.mechanicManager"),

        // ── @kit.MediaKit ───────────────────────────────────────────────────────
        ("@kit.MediaKit", _) => Some("@ohos:multimedia.media"),

        // ── @kit.MediaLibraryKit ────────────────────────────────────────────────
        ("@kit.MediaLibraryKit", "AlbumInfo") => Some("@ohos:file.AlbumPickerComponent"),
        ("@kit.MediaLibraryKit", "AlbumPickerComponent") => Some("@ohos:file.AlbumPickerComponent"),
        ("@kit.MediaLibraryKit", "AlbumPickerController") => {
            Some("@ohos:file.AlbumPickerComponent")
        }
        ("@kit.MediaLibraryKit", "AlbumPickerOptions") => Some("@ohos:file.AlbumPickerComponent"),
        ("@kit.MediaLibraryKit", "AnimatorParams") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "BaseItemInfo") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "ClickType") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "CurrentAlbumDeletedCallback") => {
            Some("@ohos:file.PhotoPickerComponent")
        }
        ("@kit.MediaLibraryKit", "DataType") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "EmptyAreaClickCallback") => {
            Some("@ohos:file.AlbumPickerComponent")
        }
        ("@kit.MediaLibraryKit", "ExceedMaxSelectedCallback") => {
            Some("@ohos:file.PhotoPickerComponent")
        }
        ("@kit.MediaLibraryKit", "ItemInfo") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "ItemType") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "ItemsDeletedCallback") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "MaxCountType") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "MaxSelected") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "MovingPhotoView") => Some("@ohos:multimedia.movingphotoview"),
        ("@kit.MediaLibraryKit", "MovingPhotoViewAttribute") => {
            Some("@ohos:multimedia.movingphotoview")
        }
        ("@kit.MediaLibraryKit", "MovingPhotoViewController") => {
            Some("@ohos:multimedia.movingphotoview")
        }
        ("@kit.MediaLibraryKit", "PhotoBrowserInfo") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "PhotoBrowserRange") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "PhotoBrowserUIElement") => {
            Some("@ohos:file.PhotoPickerComponent")
        }
        ("@kit.MediaLibraryKit", "PhotoPickerComponent") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "PhotoSource") => Some("@ohos:file.RecentPhotoComponent"),
        ("@kit.MediaLibraryKit", "PickerColorMode") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "PickerController") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "PickerOptions") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "PickerOrientation") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "RecentPhotoCheckResultCallback") => {
            Some("@ohos:file.RecentPhotoComponent")
        }
        ("@kit.MediaLibraryKit", "RecentPhotoClickCallback") => {
            Some("@ohos:file.RecentPhotoComponent")
        }
        ("@kit.MediaLibraryKit", "RecentPhotoComponent") => Some("@ohos:file.RecentPhotoComponent"),
        ("@kit.MediaLibraryKit", "RecentPhotoOptions") => Some("@ohos:file.RecentPhotoComponent"),
        ("@kit.MediaLibraryKit", "ReminderMode") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "SelectMode") => Some("@ohos:file.PhotoPickerComponent"),
        ("@kit.MediaLibraryKit", "photoAccessHelper") => Some("@ohos:file.photoAccessHelper"),
        ("@kit.MediaLibraryKit", "sendablePhotoAccessHelper") => {
            Some("@ohos:file.sendablePhotoAccessHelper")
        }

        // ── @kit.MindSporeLiteKit ───────────────────────────────────────────────
        ("@kit.MindSporeLiteKit", _) => Some("@ohos:ai.mindSporeLite"),

        // ── @kit.MultimodalAwarenessKit ─────────────────────────────────────────
        ("@kit.MultimodalAwarenessKit", "deviceStatus") => {
            Some("@ohos:multimodalAwareness.deviceStatus")
        }
        ("@kit.MultimodalAwarenessKit", "metadataBinding") => {
            Some("@ohos:multimodalAwareness.metadataBinding")
        }
        ("@kit.MultimodalAwarenessKit", "motion") => Some("@ohos:multimodalAwareness.motion"),
        ("@kit.MultimodalAwarenessKit", "onScreen") => Some("@ohos:multimodalAwareness.onScreen"),
        ("@kit.MultimodalAwarenessKit", "stationary") => Some("@ohos:stationary"),
        ("@kit.MultimodalAwarenessKit", "userStatus") => {
            Some("@ohos:multimodalAwareness.userStatus")
        }

        // ── @kit.NetworkKit ─────────────────────────────────────────────────────
        ("@kit.NetworkKit", "VpnExtensionAbility") => Some("@ohos:app.ability.VpnExtensionAbility"),
        ("@kit.NetworkKit", "VpnExtensionContext") => Some("@ohos:app.ability.VpnExtensionAbility"),
        ("@kit.NetworkKit", "connection") => Some("@ohos:net.connection"),
        ("@kit.NetworkKit", "eap") => Some("@ohos:net.eap"),
        ("@kit.NetworkKit", "ethernet") => Some("@ohos:net.ethernet"),
        ("@kit.NetworkKit", "http") => Some("@ohos:net.http"),
        ("@kit.NetworkKit", "mdns") => Some("@ohos:net.mdns"),
        ("@kit.NetworkKit", "netFirewall") => Some("@ohos:net.netFirewall"),
        ("@kit.NetworkKit", "networkSecurity") => Some("@ohos:net.networkSecurity"),
        ("@kit.NetworkKit", "policy") => Some("@ohos:net.policy"),
        ("@kit.NetworkKit", "sharing") => Some("@ohos:net.sharing"),
        ("@kit.NetworkKit", "socket") => Some("@ohos:net.socket"),
        ("@kit.NetworkKit", "statistics") => Some("@ohos:net.statistics"),
        ("@kit.NetworkKit", "vpn") => Some("@ohos:net.vpn"),
        ("@kit.NetworkKit", "vpnExtension") => Some("@ohos:net.vpnExtension"),
        ("@kit.NetworkKit", "webSocket") => Some("@ohos:net.webSocket"),

        // ── @kit.NotificationKit ────────────────────────────────────────────────
        ("@kit.NotificationKit", "ActionResult") => Some("@system:notification"),
        ("@kit.NotificationKit", "Notification") => Some("@system:notification"),
        ("@kit.NotificationKit", "ShowNotificationOptions") => Some("@system:notification"),
        ("@kit.NotificationKit", "notificationManager") => Some("@ohos:notificationManager"),

        // ── @kit.PerformanceAnalysisKit ─────────────────────────────────────────
        ("@kit.PerformanceAnalysisKit", "FaultLogExtensionAbility") => {
            Some("@ohos:hiviewdfx.FaultLogExtensionAbility")
        }
        ("@kit.PerformanceAnalysisKit", "FaultLogExtensionContext") => {
            Some("@ohos:hiviewdfx.FaultLogExtensionContext")
        }
        ("@kit.PerformanceAnalysisKit", "FaultLogger") => Some("@ohos:faultLogger"),
        ("@kit.PerformanceAnalysisKit", "hiAppEvent") => Some("@ohos:hiviewdfx.hiAppEvent"),
        ("@kit.PerformanceAnalysisKit", "hiTraceChain") => Some("@ohos:hiTraceChain"),
        ("@kit.PerformanceAnalysisKit", "hiTraceMeter") => Some("@ohos:hiTraceMeter"),
        ("@kit.PerformanceAnalysisKit", "hichecker") => Some("@ohos:hichecker"),
        ("@kit.PerformanceAnalysisKit", "hidebug") => Some("@ohos:hidebug"),
        ("@kit.PerformanceAnalysisKit", "hilog") => Some("@ohos:hilog"),
        ("@kit.PerformanceAnalysisKit", "jsLeakWatcher") => Some("@ohos:hiviewdfx.jsLeakWatcher"),

        // ── @kit.SensorServiceKit ───────────────────────────────────────────────
        ("@kit.SensorServiceKit", "AccelerometerResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "BarometerResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "CompassResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "DeviceOrientationResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "GetOnBodyStateOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "GyroscopeResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "HeartRateResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "LightResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "OnBodyStateResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "ProximityResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "Sensor") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "StepCounterResponse") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeBarometerOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeCompassOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeDeviceOrientationOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeGyroscopeOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeHeartRateOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeLightOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeOnBodyStateOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeProximityOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "SubscribeStepCounterOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "VibrateOptions") => Some("@system:vibrator"),
        ("@kit.SensorServiceKit", "Vibrator") => Some("@system:vibrator"),
        ("@kit.SensorServiceKit", "sensor") => Some("@ohos:sensor"),
        ("@kit.SensorServiceKit", "subscribeAccelerometerOptions") => Some("@system:sensor"),
        ("@kit.SensorServiceKit", "vibrator") => Some("@ohos:vibrator"),

        // ── @kit.TelephonyKit ───────────────────────────────────────────────────
        ("@kit.TelephonyKit", "call") => Some("@ohos:telephony.call"),
        ("@kit.TelephonyKit", "data") => Some("@ohos:telephony.data"),
        ("@kit.TelephonyKit", "eSIM") => Some("@ohos:telephony.esim"),
        ("@kit.TelephonyKit", "observer") => Some("@ohos:telephony.observer"),
        ("@kit.TelephonyKit", "radio") => Some("@ohos:telephony.radio"),
        ("@kit.TelephonyKit", "sim") => Some("@ohos:telephony.sim"),
        ("@kit.TelephonyKit", "sms") => Some("@ohos:telephony.sms"),
        ("@kit.TelephonyKit", "vcard") => Some("@ohos:telephony.vcard"),

        // ── @kit.TestKit ────────────────────────────────────────────────────────
        ("@kit.TestKit", "BY") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "By") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "Component") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "DisplayRotation") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "Driver") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "InputTextMode") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "MatchPattern") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "MouseButton") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "ON") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "On") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "PerfMeasureResult") => Some("@ohos:test.PerfTest"),
        ("@kit.TestKit", "PerfMetric") => Some("@ohos:test.PerfTest"),
        ("@kit.TestKit", "PerfTest") => Some("@ohos:test.PerfTest"),
        ("@kit.TestKit", "PerfTestStrategy") => Some("@ohos:test.PerfTest"),
        ("@kit.TestKit", "Point") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "PointerMatrix") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "Rect") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "ResizeDirection") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "TestRunner") => Some("@ohos:application.testRunner"),
        ("@kit.TestKit", "TouchPadSwipeOptions") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "UIElementInfo") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "UIEventObserver") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "UiComponent") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "UiDirection") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "UiDriver") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "UiWindow") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "WindowFilter") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "WindowMode") => Some("@ohos:UiTest"),
        ("@kit.TestKit", "abilityDelegatorRegistry") => {
            Some("@ohos:app.ability.abilityDelegatorRegistry")
        }

        // ── @kit.UniversalKeystoreKit ───────────────────────────────────────────
        ("@kit.UniversalKeystoreKit", _) => Some("@ohos:security.huks"),

        // ── @kit.UserAuthenticationKit ──────────────────────────────────────────
        ("@kit.UserAuthenticationKit", "UserAuthIcon") => Some("@ohos:userIAM.userAuthIcon"),
        ("@kit.UserAuthenticationKit", "faceAuth") => Some("@ohos:userIAM.faceAuth"),
        ("@kit.UserAuthenticationKit", "userAccessCtrl") => Some("@ohos:userIAM.userAccessCtrl"),
        ("@kit.UserAuthenticationKit", "userAuth") => Some("@ohos:userIAM.userAuth"),

        _ => None,
    }
}

/// Returns `true` if `spec` is a `@kit.XxxKit` module specifier.
pub fn is_kit_specifier(spec: &str) -> bool {
    spec.starts_with("@kit.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ability_kit_ui_ability() {
        assert_eq!(
            resolve_kit_import("@kit.AbilityKit", "UIAbility"),
            Some("@ohos:app.ability.UIAbility")
        );
    }

    #[test]
    fn test_ability_kit_want() {
        assert_eq!(
            resolve_kit_import("@kit.AbilityKit", "Want"),
            Some("@ohos:app.ability.Want")
        );
    }

    #[test]
    fn test_performance_kit_hilog() {
        assert_eq!(
            resolve_kit_import("@kit.PerformanceAnalysisKit", "hilog"),
            Some("@ohos:hilog")
        );
    }

    #[test]
    fn test_performance_kit_hi_app_event() {
        assert_eq!(
            resolve_kit_import("@kit.PerformanceAnalysisKit", "hiAppEvent"),
            Some("@ohos:hiviewdfx.hiAppEvent")
        );
    }

    #[test]
    fn test_core_file_kit_file_io() {
        assert_eq!(
            resolve_kit_import("@kit.CoreFileKit", "fileIo"),
            Some("@ohos:file.fs")
        );
    }

    #[test]
    fn test_core_file_kit_hash() {
        assert_eq!(
            resolve_kit_import("@kit.CoreFileKit", "hash"),
            Some("@ohos:file.hash")
        );
    }

    #[test]
    fn test_network_kit_http() {
        assert_eq!(
            resolve_kit_import("@kit.NetworkKit", "http"),
            Some("@ohos:net.http")
        );
    }

    #[test]
    fn test_network_kit_socket() {
        assert_eq!(
            resolve_kit_import("@kit.NetworkKit", "socket"),
            Some("@ohos:net.socket")
        );
    }

    #[test]
    fn test_network_kit_connection() {
        assert_eq!(
            resolve_kit_import("@kit.NetworkKit", "connection"),
            Some("@ohos:net.connection")
        );
    }

    #[test]
    fn test_basic_services_kit_common_event() {
        assert_eq!(
            resolve_kit_import("@kit.BasicServicesKit", "commonEventManager"),
            Some("@ohos:commonEventManager")
        );
    }

    #[test]
    fn test_arkui_kit_window() {
        assert_eq!(
            resolve_kit_import("@kit.ArkUI", "window"),
            Some("@ohos:window")
        );
    }

    #[test]
    fn test_arkts_kit_util() {
        assert_eq!(resolve_kit_import("@kit.ArkTS", "util"), Some("@ohos:util"));
    }

    #[test]
    fn test_connectivity_kit_bluetooth() {
        assert_eq!(
            resolve_kit_import("@kit.ConnectivityKit", "bluetooth"),
            Some("@ohos:bluetooth")
        );
    }

    #[test]
    fn test_telephony_kit_call() {
        assert_eq!(
            resolve_kit_import("@kit.TelephonyKit", "call"),
            Some("@ohos:telephony.call")
        );
    }

    #[test]
    fn test_audio_kit_audio() {
        assert_eq!(
            resolve_kit_import("@kit.AudioKit", "audio"),
            Some("@ohos:multimedia.audio")
        );
    }

    #[test]
    fn test_camera_kit_camera() {
        assert_eq!(
            resolve_kit_import("@kit.CameraKit", "camera"),
            Some("@ohos:multimedia.camera")
        );
    }

    #[test]
    fn test_image_kit_image() {
        assert_eq!(
            resolve_kit_import("@kit.ImageKit", "image"),
            Some("@ohos:multimedia.image")
        );
    }

    #[test]
    fn test_location_kit_geo() {
        assert_eq!(
            resolve_kit_import("@kit.LocationKit", "geoLocationManager"),
            Some("@ohos:geoLocationManager")
        );
    }

    #[test]
    fn test_single_module_kits() {
        // Kits where every symbol maps to the same module
        assert_eq!(
            resolve_kit_import("@kit.CalendarKit", "calendarManager"),
            Some("@ohos:calendarManager")
        );
        assert_eq!(
            resolve_kit_import("@kit.ContactsKit", "contact"),
            Some("@ohos:contact")
        );
        assert_eq!(resolve_kit_import("@kit.IPCKit", "rpc"), Some("@ohos:rpc"));
        assert_eq!(
            resolve_kit_import("@kit.MediaKit", "media"),
            Some("@ohos:multimedia.media")
        );
        assert_eq!(
            resolve_kit_import("@kit.DrmKit", "drm"),
            Some("@ohos:multimedia.drm")
        );
        assert_eq!(
            resolve_kit_import("@kit.UniversalKeystoreKit", "huks"),
            Some("@ohos:security.huks")
        );
        assert_eq!(
            resolve_kit_import("@kit.AssetStoreKit", "asset"),
            Some("@ohos:security.asset")
        );
    }

    #[test]
    fn test_unknown_kit_returns_none() {
        assert_eq!(resolve_kit_import("@kit.UnknownKit", "something"), None);
    }

    #[test]
    fn test_is_kit_specifier() {
        assert!(is_kit_specifier("@kit.AbilityKit"));
        assert!(is_kit_specifier("@kit.ArkUI"));
        assert!(!is_kit_specifier("@ohos:hilog"));
        assert!(!is_kit_specifier("@ohos/hilog"));
    }
}
