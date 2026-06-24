//! Maps `@kit.XxxKit` named imports to their source module paths.
//!
//! Auto-generated from OpenHarmony SDK kit_configs/*.json and HMS kit_configs/*.json.
//! Run `scripts/gen_kit_map.py` to regenerate.

use arkdown_path::ModuleSpecifierText;

/// A resolved kit import binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KitEntry {
    /// The symbol is a default export from its module.
    Default(&'static str),
    /// The symbol is a named export. Field 1 = module, field 2 = binding name.
    Named(&'static str, &'static str),
}

impl KitEntry {
    /// The module path (e.g. `@ohos:hilog`).
    pub fn module(&self) -> &'static str {
        match self {
            KitEntry::Default(m) | KitEntry::Named(m, _) => m,
        }
    }
}

/// Resolve a single named import from a `@kit.XxxKit` specifier.
pub fn resolve_kit_import(kit: &str, sym: &str) -> Option<KitEntry> {
    match (kit, sym) {
        // ── @kit.AVSessionKit ───────────────────────────────────────────────
        ("@kit.AVSessionKit", "AVCastPicker") => {
            Some(KitEntry::Default("@ohos:multimedia.avCastPicker"))
        }
        ("@kit.AVSessionKit", "AVCastPickerColorMode") => Some(KitEntry::Named(
            "@ohos:multimedia.avCastPickerParam",
            "AVCastPickerColorMode",
        )),
        ("@kit.AVSessionKit", "AVCastPickerState") => Some(KitEntry::Named(
            "@ohos:multimedia.avCastPickerParam",
            "AVCastPickerState",
        )),
        ("@kit.AVSessionKit", "AVCastPickerStyle") => Some(KitEntry::Named(
            "@ohos:multimedia.avCastPickerParam",
            "AVCastPickerStyle",
        )),
        ("@kit.AVSessionKit", "AVInputCastPicker") => Some(KitEntry::Named(
            "@ohos:multimedia.avInputCastPicker",
            "AVInputCastPicker",
        )),
        ("@kit.AVSessionKit", "avSession") => Some(KitEntry::Default("@ohos:multimedia.avsession")),

        // ── @kit.AbilityKit ───────────────────────────────────────────────
        ("@kit.AbilityKit", "Ability") => Some(KitEntry::Default("@ohos:app.ability.Ability")),
        ("@kit.AbilityKit", "AbilityConstant") => {
            Some(KitEntry::Default("@ohos:app.ability.AbilityConstant"))
        }
        ("@kit.AbilityKit", "AbilityLifecycleCallback") => Some(KitEntry::Default(
            "@ohos:app.ability.AbilityLifecycleCallback",
        )),
        ("@kit.AbilityKit", "AbilityStage") => {
            Some(KitEntry::Default("@ohos:app.ability.AbilityStage"))
        }
        ("@kit.AbilityKit", "AbilityStartFailureCode") => Some(KitEntry::Named(
            "@ohos:app.ability.CompletionHandlerForAbilityStartCallback",
            "AbilityStartFailureCode",
        )),
        ("@kit.AbilityKit", "ActionExtensionAbility") => Some(KitEntry::Default(
            "@ohos:app.ability.ActionExtensionAbility",
        )),
        ("@kit.AbilityKit", "AppServiceExtensionAbility") => Some(KitEntry::Default(
            "@ohos:app.ability.AppServiceExtensionAbility",
        )),
        ("@kit.AbilityKit", "ApplicationStateChangeCallback") => Some(KitEntry::Default(
            "@ohos:app.ability.ApplicationStateChangeCallback",
        )),
        ("@kit.AbilityKit", "AtomicServiceOptions") => {
            Some(KitEntry::Default("@ohos:app.ability.AtomicServiceOptions"))
        }
        ("@kit.AbilityKit", "Callee") => {
            Some(KitEntry::Named("@ohos:app.ability.UIAbility", "Callee"))
        }
        ("@kit.AbilityKit", "CalleeCallback") => Some(KitEntry::Named(
            "@ohos:app.ability.UIAbility",
            "CalleeCallback",
        )),
        ("@kit.AbilityKit", "Caller") => {
            Some(KitEntry::Named("@ohos:app.ability.UIAbility", "Caller"))
        }
        ("@kit.AbilityKit", "CheckPackageHasInstalledOptions") => Some(KitEntry::Named(
            "@system:package",
            "CheckPackageHasInstalledOptions",
        )),
        ("@kit.AbilityKit", "CheckPackageHasInstalledResponse") => Some(KitEntry::Named(
            "@system:package",
            "CheckPackageHasInstalledResponse",
        )),
        ("@kit.AbilityKit", "ChildProcess") => {
            Some(KitEntry::Default("@ohos:app.ability.ChildProcess"))
        }
        ("@kit.AbilityKit", "ChildProcessArgs") => Some(KitEntry::Named(
            "@ohos:app.ability.ChildProcessArgs",
            "ChildProcessArgs",
        )),
        ("@kit.AbilityKit", "ChildProcessOptions") => Some(KitEntry::Named(
            "@ohos:app.ability.ChildProcessOptions",
            "ChildProcessOptions",
        )),
        ("@kit.AbilityKit", "CompletionHandler") => {
            Some(KitEntry::Default("@ohos:app.ability.CompletionHandler"))
        }
        ("@kit.AbilityKit", "CompletionHandlerForAbilityStartCallback") => Some(KitEntry::Named(
            "@ohos:app.ability.CompletionHandlerForAbilityStartCallback",
            "CompletionHandlerForAbilityStartCallback",
        )),
        ("@kit.AbilityKit", "CompletionHandlerForAtomicService") => Some(KitEntry::Default(
            "@ohos:app.ability.CompletionHandlerForAtomicService",
        )),
        ("@kit.AbilityKit", "Configuration") => Some(KitEntry::Named(
            "@ohos:app.ability.Configuration",
            "Configuration",
        )),
        ("@kit.AbilityKit", "ConfigurationConstant") => {
            Some(KitEntry::Default("@ohos:app.ability.ConfigurationConstant"))
        }
        ("@kit.AbilityKit", "Context") => {
            Some(KitEntry::Named("@ohos:abilityAccessCtrl", "Context"))
        }
        ("@kit.AbilityKit", "EmbeddableUIAbility") => {
            Some(KitEntry::Default("@ohos:app.ability.EmbeddableUIAbility"))
        }
        ("@kit.AbilityKit", "EmbeddedUIExtensionAbility") => Some(KitEntry::Default(
            "@ohos:app.ability.EmbeddedUIExtensionAbility",
        )),
        ("@kit.AbilityKit", "EnvironmentCallback") => {
            Some(KitEntry::Default("@ohos:app.ability.EnvironmentCallback"))
        }
        ("@kit.AbilityKit", "ErrorCode") => {
            Some(KitEntry::Named("@ohos:ability.errorCode", "ErrorCode"))
        }
        ("@kit.AbilityKit", "ExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.ability.ExtensionAbility"))
        }
        ("@kit.AbilityKit", "InsightIntentContext") => {
            Some(KitEntry::Default("@ohos:app.ability.InsightIntentContext"))
        }
        ("@kit.AbilityKit", "InsightIntentEntity") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "InsightIntentEntity",
        )),
        ("@kit.AbilityKit", "InsightIntentEntry") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "InsightIntentEntry",
        )),
        ("@kit.AbilityKit", "InsightIntentEntryExecutor") => Some(KitEntry::Default(
            "@ohos:app.ability.InsightIntentEntryExecutor",
        )),
        ("@kit.AbilityKit", "InsightIntentExecutor") => {
            Some(KitEntry::Default("@ohos:app.ability.InsightIntentExecutor"))
        }
        ("@kit.AbilityKit", "InsightIntentForm") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "InsightIntentForm",
        )),
        ("@kit.AbilityKit", "InsightIntentFunction") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "InsightIntentFunction",
        )),
        ("@kit.AbilityKit", "InsightIntentFunctionMethod") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "InsightIntentFunctionMethod",
        )),
        ("@kit.AbilityKit", "InsightIntentLink") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "InsightIntentLink",
        )),
        ("@kit.AbilityKit", "InsightIntentPage") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "InsightIntentPage",
        )),
        ("@kit.AbilityKit", "LinkParamCategory") => Some(KitEntry::Named(
            "@ohos:app.ability.InsightIntentDecorator",
            "LinkParamCategory",
        )),
        ("@kit.AbilityKit", "OnReleaseCallback") => Some(KitEntry::Named(
            "@ohos:app.ability.UIAbility",
            "OnReleaseCallback",
        )),
        ("@kit.AbilityKit", "OnRemoteStateChangeCallback") => Some(KitEntry::Named(
            "@ohos:app.ability.UIAbility",
            "OnRemoteStateChangeCallback",
        )),
        ("@kit.AbilityKit", "OpenLinkOptions") => {
            Some(KitEntry::Default("@ohos:app.ability.OpenLinkOptions"))
        }
        ("@kit.AbilityKit", "Package") => Some(KitEntry::Default("@system:package")),
        ("@kit.AbilityKit", "PermissionRequestResult") => Some(KitEntry::Named(
            "@ohos:abilityAccessCtrl",
            "PermissionRequestResult",
        )),
        ("@kit.AbilityKit", "Permissions") => {
            Some(KitEntry::Named("@ohos:abilityAccessCtrl", "Permissions"))
        }
        ("@kit.AbilityKit", "PhotoEditorExtensionAbility") => Some(KitEntry::Default(
            "@ohos:app.ability.PhotoEditorExtensionAbility",
        )),
        ("@kit.AbilityKit", "ShareExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.ability.ShareExtensionAbility"))
        }
        ("@kit.AbilityKit", "StartOptions") => {
            Some(KitEntry::Default("@ohos:app.ability.StartOptions"))
        }
        ("@kit.AbilityKit", "StartupConfig") => {
            Some(KitEntry::Default("@ohos:app.appstartup.StartupConfig"))
        }
        ("@kit.AbilityKit", "StartupConfigEntry") => {
            Some(KitEntry::Default("@ohos:app.appstartup.StartupConfigEntry"))
        }
        ("@kit.AbilityKit", "StartupListener") => {
            Some(KitEntry::Default("@ohos:app.appstartup.StartupListener"))
        }
        ("@kit.AbilityKit", "StartupTask") => {
            Some(KitEntry::Default("@ohos:app.appstartup.StartupTask"))
        }
        ("@kit.AbilityKit", "UIAbility") => Some(KitEntry::Default("@ohos:app.ability.UIAbility")),
        ("@kit.AbilityKit", "UIExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.ability.UIExtensionAbility"))
        }
        ("@kit.AbilityKit", "UIExtensionContentSession") => Some(KitEntry::Default(
            "@ohos:app.ability.UIExtensionContentSession",
        )),
        ("@kit.AbilityKit", "Want") => Some(KitEntry::Default("@ohos:app.ability.Want")),
        ("@kit.AbilityKit", "WantAgent") => {
            Some(KitEntry::Named("@ohos:app.ability.wantAgent", "WantAgent"))
        }
        ("@kit.AbilityKit", "ability") => Some(KitEntry::Default("@ohos:ability.ability")),
        ("@kit.AbilityKit", "abilityAccessCtrl") => {
            Some(KitEntry::Default("@ohos:abilityAccessCtrl"))
        }
        ("@kit.AbilityKit", "abilityManager") => {
            Some(KitEntry::Default("@ohos:app.ability.abilityManager"))
        }
        ("@kit.AbilityKit", "appManager") => {
            Some(KitEntry::Default("@ohos:app.ability.appManager"))
        }
        ("@kit.AbilityKit", "appRecovery") => {
            Some(KitEntry::Default("@ohos:app.ability.appRecovery"))
        }
        ("@kit.AbilityKit", "application") => {
            Some(KitEntry::Default("@ohos:app.ability.application"))
        }
        ("@kit.AbilityKit", "autoFillManager") => {
            Some(KitEntry::Default("@ohos:app.ability.autoFillManager"))
        }
        ("@kit.AbilityKit", "autoStartupManager") => {
            Some(KitEntry::Default("@ohos:app.ability.autoStartupManager"))
        }
        ("@kit.AbilityKit", "bundle") => Some(KitEntry::Default("@ohos:bundle")),
        ("@kit.AbilityKit", "bundleManager") => {
            Some(KitEntry::Default("@ohos:bundle.bundleManager"))
        }
        ("@kit.AbilityKit", "childProcessManager") => {
            Some(KitEntry::Default("@ohos:app.ability.childProcessManager"))
        }
        ("@kit.AbilityKit", "common") => Some(KitEntry::Default("@ohos:app.ability.common")),
        ("@kit.AbilityKit", "contextConstant") => {
            Some(KitEntry::Default("@ohos:app.ability.contextConstant"))
        }
        ("@kit.AbilityKit", "continuationManager") => {
            Some(KitEntry::Default("@ohos:continuation.continuationManager"))
        }
        ("@kit.AbilityKit", "continueManager") => {
            Some(KitEntry::Default("@ohos:app.ability.continueManager"))
        }
        ("@kit.AbilityKit", "dataUriUtils") => {
            Some(KitEntry::Default("@ohos:app.ability.dataUriUtils"))
        }
        ("@kit.AbilityKit", "defaultAppManager") => {
            Some(KitEntry::Default("@ohos:bundle.defaultAppManager"))
        }
        ("@kit.AbilityKit", "dialogRequest") => {
            Some(KitEntry::Default("@ohos:app.ability.dialogRequest"))
        }
        ("@kit.AbilityKit", "errorManager") => {
            Some(KitEntry::Default("@ohos:app.ability.errorManager"))
        }
        ("@kit.AbilityKit", "featureAbility") => {
            Some(KitEntry::Default("@ohos:ability.featureAbility"))
        }
        ("@kit.AbilityKit", "insightIntent") => {
            Some(KitEntry::Default("@ohos:app.ability.insightIntent"))
        }
        ("@kit.AbilityKit", "kioskManager") => {
            Some(KitEntry::Default("@ohos:app.ability.kioskManager"))
        }
        ("@kit.AbilityKit", "launcherBundleManager") => {
            Some(KitEntry::Default("@ohos:bundle.launcherBundleManager"))
        }
        ("@kit.AbilityKit", "overlay") => Some(KitEntry::Default("@ohos:bundle.overlay")),
        ("@kit.AbilityKit", "particleAbility") => {
            Some(KitEntry::Default("@ohos:ability.particleAbility"))
        }
        ("@kit.AbilityKit", "privacyManager") => Some(KitEntry::Default("@ohos:privacyManager")),
        ("@kit.AbilityKit", "screenLockFileManager") => {
            Some(KitEntry::Default("@ohos:ability.screenLockFileManager"))
        }
        ("@kit.AbilityKit", "sendableContextManager") => Some(KitEntry::Default(
            "@ohos:app.ability.sendableContextManager",
        )),
        ("@kit.AbilityKit", "shortcutManager") => {
            Some(KitEntry::Default("@ohos:bundle.shortcutManager"))
        }
        ("@kit.AbilityKit", "startupManager") => {
            Some(KitEntry::Default("@ohos:app.appstartup.startupManager"))
        }
        ("@kit.AbilityKit", "uriPermissionManager") => {
            Some(KitEntry::Default("@ohos:application.uriPermissionManager"))
        }
        ("@kit.AbilityKit", "wantAgent") => Some(KitEntry::Default("@ohos:app.ability.wantAgent")),
        ("@kit.AbilityKit", "wantConstant") => {
            Some(KitEntry::Default("@ohos:app.ability.wantConstant"))
        }

        // ── @kit.AccessibilityKit ───────────────────────────────────────────────
        ("@kit.AccessibilityKit", "AccessibilityElement") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "AccessibilityElement",
        )),
        ("@kit.AccessibilityKit", "AccessibilityEvent") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "AccessibilityEvent",
        )),
        ("@kit.AccessibilityKit", "AccessibilityExtensionAbility") => Some(KitEntry::Default(
            "@ohos:application.AccessibilityExtensionAbility",
        )),
        ("@kit.AccessibilityKit", "AccessibilityExtensionContext") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "AccessibilityExtensionContext",
        )),
        ("@kit.AccessibilityKit", "ElementAttributeKeys") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "ElementAttributeKeys",
        )),
        ("@kit.AccessibilityKit", "ElementAttributeValues") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "ElementAttributeValues",
        )),
        ("@kit.AccessibilityKit", "FocusDirection") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "FocusDirection",
        )),
        ("@kit.AccessibilityKit", "FocusType") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "FocusType",
        )),
        ("@kit.AccessibilityKit", "GesturePath") => Some(KitEntry::Named(
            "@ohos:accessibility.GesturePath",
            "GesturePath",
        )),
        ("@kit.AccessibilityKit", "GesturePoint") => Some(KitEntry::Named(
            "@ohos:accessibility.GesturePoint",
            "GesturePoint",
        )),
        ("@kit.AccessibilityKit", "Rect") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "Rect",
        )),
        ("@kit.AccessibilityKit", "WindowType") => Some(KitEntry::Named(
            "@ohos:application.AccessibilityExtensionAbility",
            "WindowType",
        )),
        ("@kit.AccessibilityKit", "accessibility") => {
            Some(KitEntry::Default("@ohos:accessibility"))
        }

        // ── @kit.AdsKit ───────────────────────────────────────────────
        ("@kit.AdsKit", "AdComponent") => Some(KitEntry::Named(
            "@ohos:advertising.AdComponent",
            "AdComponent",
        )),
        ("@kit.AdsKit", "AutoAdComponent") => Some(KitEntry::Named(
            "@ohos:advertising.AutoAdComponent",
            "AutoAdComponent",
        )),
        ("@kit.AdsKit", "RespCallback") => Some(KitEntry::Named(
            "@ohos:advertising.AdsServiceExtensionAbility",
            "RespCallback",
        )),
        ("@kit.AdsKit", "advertising") => Some(KitEntry::Default("@ohos:advertising")),
        ("@kit.AdsKit", "identifier") => Some(KitEntry::Default("@ohos:identifier.oaid")),

        // ── @kit.ArkData ───────────────────────────────────────────────
        ("@kit.ArkData", "ContentFormCard") => Some(KitEntry::Named(
            "@ohos:data.UdmfComponents",
            "ContentFormCard",
        )),
        ("@kit.ArkData", "FormType") => {
            Some(KitEntry::Named("@ohos:data.UdmfComponents", "FormType"))
        }
        ("@kit.ArkData", "ValueType") => {
            Some(KitEntry::Named("@ohos:data.ValuesBucket", "ValueType"))
        }
        ("@kit.ArkData", "ValuesBucket") => {
            Some(KitEntry::Named("@ohos:data.ValuesBucket", "ValuesBucket"))
        }
        ("@kit.ArkData", "cloudData") => Some(KitEntry::Default("@ohos:data.cloudData")),
        ("@kit.ArkData", "cloudExtension") => Some(KitEntry::Default("@ohos:data.cloudExtension")),
        ("@kit.ArkData", "commonType") => Some(KitEntry::Default("@ohos:data.commonType")),
        ("@kit.ArkData", "dataAbility") => Some(KitEntry::Default("@ohos:data.dataAbility")),
        ("@kit.ArkData", "dataShare") => Some(KitEntry::Default("@ohos:data.dataShare")),
        ("@kit.ArkData", "dataSharePredicates") => {
            Some(KitEntry::Default("@ohos:data.dataSharePredicates"))
        }
        ("@kit.ArkData", "distributedDataObject") => {
            Some(KitEntry::Default("@ohos:data.distributedDataObject"))
        }
        ("@kit.ArkData", "distributedKVStore") => {
            Some(KitEntry::Default("@ohos:data.distributedKVStore"))
        }
        ("@kit.ArkData", "intelligence") => Some(KitEntry::Default("@ohos:data.intelligence")),
        ("@kit.ArkData", "preferences") => Some(KitEntry::Default("@ohos:data.preferences")),
        ("@kit.ArkData", "relationalStore") => {
            Some(KitEntry::Default("@ohos:data.relationalStore"))
        }
        ("@kit.ArkData", "sendablePreferences") => {
            Some(KitEntry::Default("@ohos:data.sendablePreferences"))
        }
        ("@kit.ArkData", "sendableRelationalStore") => {
            Some(KitEntry::Default("@ohos:data.sendableRelationalStore"))
        }
        ("@kit.ArkData", "unifiedDataChannel") => {
            Some(KitEntry::Default("@ohos:data.unifiedDataChannel"))
        }
        ("@kit.ArkData", "uniformDataStruct") => {
            Some(KitEntry::Default("@ohos:data.uniformDataStruct"))
        }
        ("@kit.ArkData", "uniformTypeDescriptor") => {
            Some(KitEntry::Default("@ohos:data.uniformTypeDescriptor"))
        }

        // ── @kit.ArkGraphics2D ───────────────────────────────────────────────
        ("@kit.ArkGraphics2D", "colorSpaceManager") => {
            Some(KitEntry::Default("@ohos:graphics.colorSpaceManager"))
        }
        ("@kit.ArkGraphics2D", "common2D") => Some(KitEntry::Default("@ohos:graphics.common2D")),
        ("@kit.ArkGraphics2D", "displaySync") => {
            Some(KitEntry::Default("@ohos:graphics.displaySync"))
        }
        ("@kit.ArkGraphics2D", "drawing") => Some(KitEntry::Default("@ohos:graphics.drawing")),
        ("@kit.ArkGraphics2D", "effectKit") => Some(KitEntry::Default("@ohos:effectKit")),
        ("@kit.ArkGraphics2D", "hdrCapability") => {
            Some(KitEntry::Default("@ohos:graphics.hdrCapability"))
        }
        ("@kit.ArkGraphics2D", "sendableColorSpaceManager") => Some(KitEntry::Default(
            "@ohos:graphics.sendableColorSpaceManager",
        )),
        ("@kit.ArkGraphics2D", "text") => Some(KitEntry::Default("@ohos:graphics.text")),
        ("@kit.ArkGraphics2D", "uiEffect") => Some(KitEntry::Default("@ohos:graphics.uiEffect")),

        // ── @kit.ArkGraphics3D ───────────────────────────────────────────────
        ("@kit.ArkGraphics3D", "Aabb") => Some(KitEntry::Named("@ohos:graphics.scene", "Aabb")),
        ("@kit.ArkGraphics3D", "Animation") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Animation"))
        }
        ("@kit.ArkGraphics3D", "Blend") => Some(KitEntry::Named("@ohos:graphics.scene", "Blend")),
        ("@kit.ArkGraphics3D", "BloomSettings") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "BloomSettings"))
        }
        ("@kit.ArkGraphics3D", "Camera") => Some(KitEntry::Named("@ohos:graphics.scene", "Camera")),
        ("@kit.ArkGraphics3D", "CameraParameters") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "CameraParameters"))
        }
        ("@kit.ArkGraphics3D", "Color") => Some(KitEntry::Named("@ohos:graphics.scene", "Color")),
        ("@kit.ArkGraphics3D", "Container") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Container"))
        }
        ("@kit.ArkGraphics3D", "CubeGeometry") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "CubeGeometry"))
        }
        ("@kit.ArkGraphics3D", "CullMode") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "CullMode"))
        }
        ("@kit.ArkGraphics3D", "CustomGeometry") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "CustomGeometry"))
        }
        ("@kit.ArkGraphics3D", "DirectionalLight") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "DirectionalLight"))
        }
        ("@kit.ArkGraphics3D", "Effect") => Some(KitEntry::Named("@ohos:graphics.scene", "Effect")),
        ("@kit.ArkGraphics3D", "EffectParameters") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "EffectParameters"))
        }
        ("@kit.ArkGraphics3D", "Environment") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Environment"))
        }
        ("@kit.ArkGraphics3D", "EnvironmentBackgroundType") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "EnvironmentBackgroundType",
        )),
        ("@kit.ArkGraphics3D", "Geometry") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Geometry"))
        }
        ("@kit.ArkGraphics3D", "GeometryDefinition") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "GeometryDefinition",
        )),
        ("@kit.ArkGraphics3D", "GeometryType") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "GeometryType"))
        }
        ("@kit.ArkGraphics3D", "Image") => Some(KitEntry::Named("@ohos:graphics.scene", "Image")),
        ("@kit.ArkGraphics3D", "LayerMask") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "LayerMask"))
        }
        ("@kit.ArkGraphics3D", "Light") => Some(KitEntry::Named("@ohos:graphics.scene", "Light")),
        ("@kit.ArkGraphics3D", "LightType") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "LightType"))
        }
        ("@kit.ArkGraphics3D", "Material") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Material"))
        }
        ("@kit.ArkGraphics3D", "MaterialProperty") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "MaterialProperty"))
        }
        ("@kit.ArkGraphics3D", "MaterialType") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "MaterialType"))
        }
        ("@kit.ArkGraphics3D", "Mesh") => Some(KitEntry::Named("@ohos:graphics.scene", "Mesh")),
        ("@kit.ArkGraphics3D", "MeshResource") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "MeshResource"))
        }
        ("@kit.ArkGraphics3D", "MetallicRoughnessMaterial") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "MetallicRoughnessMaterial",
        )),
        ("@kit.ArkGraphics3D", "Morpher") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Morpher"))
        }
        ("@kit.ArkGraphics3D", "Node") => Some(KitEntry::Named("@ohos:graphics.scene", "Node")),
        ("@kit.ArkGraphics3D", "NodeType") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "NodeType"))
        }
        ("@kit.ArkGraphics3D", "PlaneGeometry") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "PlaneGeometry"))
        }
        ("@kit.ArkGraphics3D", "Position3") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Position3"))
        }
        ("@kit.ArkGraphics3D", "PostProcessSettings") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "PostProcessSettings",
        )),
        ("@kit.ArkGraphics3D", "PrimitiveTopology") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "PrimitiveTopology"))
        }
        ("@kit.ArkGraphics3D", "Quaternion") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Quaternion"))
        }
        ("@kit.ArkGraphics3D", "RaycastParameters") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "RaycastParameters"))
        }
        ("@kit.ArkGraphics3D", "RaycastResult") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "RaycastResult"))
        }
        ("@kit.ArkGraphics3D", "Rect") => Some(KitEntry::Named("@ohos:graphics.scene", "Rect")),
        ("@kit.ArkGraphics3D", "RenderContext") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "RenderContext"))
        }
        ("@kit.ArkGraphics3D", "RenderParameters") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "RenderParameters"))
        }
        ("@kit.ArkGraphics3D", "RenderResourceFactory") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "RenderResourceFactory",
        )),
        ("@kit.ArkGraphics3D", "RenderSort") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "RenderSort"))
        }
        ("@kit.ArkGraphics3D", "RenderingPipelineType") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "RenderingPipelineType",
        )),
        ("@kit.ArkGraphics3D", "Rotation3") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Rotation3"))
        }
        ("@kit.ArkGraphics3D", "Sampler") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "Sampler"))
        }
        ("@kit.ArkGraphics3D", "SamplerAddressMode") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "SamplerAddressMode",
        )),
        ("@kit.ArkGraphics3D", "SamplerFilter") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "SamplerFilter"))
        }
        ("@kit.ArkGraphics3D", "Scale3") => Some(KitEntry::Named("@ohos:graphics.scene", "Scale3")),
        ("@kit.ArkGraphics3D", "Scene") => Some(KitEntry::Named("@ohos:graphics.scene", "Scene")),
        ("@kit.ArkGraphics3D", "SceneComponent") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "SceneComponent"))
        }
        ("@kit.ArkGraphics3D", "SceneNodeParameters") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "SceneNodeParameters",
        )),
        ("@kit.ArkGraphics3D", "SceneResource") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "SceneResource"))
        }
        ("@kit.ArkGraphics3D", "SceneResourceFactory") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "SceneResourceFactory",
        )),
        ("@kit.ArkGraphics3D", "SceneResourceParameters") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "SceneResourceParameters",
        )),
        ("@kit.ArkGraphics3D", "SceneResourceType") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "SceneResourceType"))
        }
        ("@kit.ArkGraphics3D", "Shader") => Some(KitEntry::Named("@ohos:graphics.scene", "Shader")),
        ("@kit.ArkGraphics3D", "ShaderMaterial") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "ShaderMaterial"))
        }
        ("@kit.ArkGraphics3D", "SphereGeometry") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "SphereGeometry"))
        }
        ("@kit.ArkGraphics3D", "SpotLight") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "SpotLight"))
        }
        ("@kit.ArkGraphics3D", "SubMesh") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "SubMesh"))
        }
        ("@kit.ArkGraphics3D", "ToneMappingSettings") => Some(KitEntry::Named(
            "@ohos:graphics.scene",
            "ToneMappingSettings",
        )),
        ("@kit.ArkGraphics3D", "ToneMappingType") => {
            Some(KitEntry::Named("@ohos:graphics.scene", "ToneMappingType"))
        }
        ("@kit.ArkGraphics3D", "Vec2") => Some(KitEntry::Named("@ohos:graphics.scene", "Vec2")),
        ("@kit.ArkGraphics3D", "Vec3") => Some(KitEntry::Named("@ohos:graphics.scene", "Vec3")),
        ("@kit.ArkGraphics3D", "Vec4") => Some(KitEntry::Named("@ohos:graphics.scene", "Vec4")),

        // ── @kit.ArkTS ───────────────────────────────────────────────
        ("@kit.ArkTS", "ArkTSUtils") => Some(KitEntry::Default("@arkts.utils")),
        ("@kit.ArkTS", "ArrayList") => Some(KitEntry::Default("@ohos:util.ArrayList")),
        ("@kit.ArkTS", "Decimal") => Some(KitEntry::Default("@arkts.math.Decimal")),
        ("@kit.ArkTS", "DedicatedWorkerGlobalScope") => Some(KitEntry::Named(
            "@ohos:worker",
            "DedicatedWorkerGlobalScope",
        )),
        ("@kit.ArkTS", "Deque") => Some(KitEntry::Default("@ohos:util.Deque")),
        ("@kit.ArkTS", "ErrorEvent") => Some(KitEntry::Named("@ohos:worker", "ErrorEvent")),
        ("@kit.ArkTS", "Event") => Some(KitEntry::Named("@ohos:worker", "Event")),
        ("@kit.ArkTS", "EventListener") => Some(KitEntry::Named("@ohos:worker", "EventListener")),
        ("@kit.ArkTS", "EventTarget") => Some(KitEntry::Named("@ohos:worker", "EventTarget")),
        ("@kit.ArkTS", "HashMap") => Some(KitEntry::Default("@ohos:util.HashMap")),
        ("@kit.ArkTS", "HashSet") => Some(KitEntry::Default("@ohos:util.HashSet")),
        ("@kit.ArkTS", "JSON") => Some(KitEntry::Default("@ohos:util.json")),
        ("@kit.ArkTS", "LightWeightMap") => Some(KitEntry::Default("@ohos:util.LightWeightMap")),
        ("@kit.ArkTS", "LightWeightSet") => Some(KitEntry::Default("@ohos:util.LightWeightSet")),
        ("@kit.ArkTS", "LinkedList") => Some(KitEntry::Default("@ohos:util.LinkedList")),
        ("@kit.ArkTS", "List") => Some(KitEntry::Default("@ohos:util.List")),
        ("@kit.ArkTS", "MessageEvent") => Some(KitEntry::Named("@ohos:worker", "MessageEvent")),
        ("@kit.ArkTS", "MessageEvents") => Some(KitEntry::Named("@ohos:worker", "MessageEvents")),
        ("@kit.ArkTS", "PlainArray") => Some(KitEntry::Default("@ohos:util.PlainArray")),
        ("@kit.ArkTS", "PostMessageOptions") => {
            Some(KitEntry::Named("@ohos:worker", "PostMessageOptions"))
        }
        ("@kit.ArkTS", "Queue") => Some(KitEntry::Default("@ohos:util.Queue")),
        ("@kit.ArkTS", "Stack") => Some(KitEntry::Default("@ohos:util.Stack")),
        ("@kit.ArkTS", "ThreadWorkerGlobalScope") => {
            Some(KitEntry::Named("@ohos:worker", "ThreadWorkerGlobalScope"))
        }
        ("@kit.ArkTS", "ThreadWorkerPriority") => {
            Some(KitEntry::Named("@ohos:worker", "ThreadWorkerPriority"))
        }
        ("@kit.ArkTS", "TreeMap") => Some(KitEntry::Default("@ohos:util.TreeMap")),
        ("@kit.ArkTS", "TreeSet") => Some(KitEntry::Default("@ohos:util.TreeSet")),
        ("@kit.ArkTS", "Vector") => Some(KitEntry::Default("@ohos:util.Vector")),
        ("@kit.ArkTS", "WorkerEventListener") => {
            Some(KitEntry::Named("@ohos:worker", "WorkerEventListener"))
        }
        ("@kit.ArkTS", "WorkerEventTarget") => {
            Some(KitEntry::Named("@ohos:worker", "WorkerEventTarget"))
        }
        ("@kit.ArkTS", "WorkerOptions") => Some(KitEntry::Named("@ohos:worker", "WorkerOptions")),
        ("@kit.ArkTS", "buffer") => Some(KitEntry::Default("@ohos:buffer")),
        ("@kit.ArkTS", "collections") => Some(KitEntry::Default("@arkts.collections")),
        ("@kit.ArkTS", "convertxml") => Some(KitEntry::Default("@ohos:convertxml")),
        ("@kit.ArkTS", "fastbuffer") => Some(KitEntry::Default("@ohos:fastbuffer")),
        ("@kit.ArkTS", "lang") => Some(KitEntry::Default("@arkts.lang")),
        ("@kit.ArkTS", "process") => Some(KitEntry::Default("@ohos:process")),
        ("@kit.ArkTS", "stream") => Some(KitEntry::Default("@ohos:util.stream")),
        ("@kit.ArkTS", "taskpool") => Some(KitEntry::Default("@ohos:taskpool")),
        ("@kit.ArkTS", "uri") => Some(KitEntry::Default("@ohos:uri")),
        ("@kit.ArkTS", "url") => Some(KitEntry::Default("@ohos:url")),
        ("@kit.ArkTS", "util") => Some(KitEntry::Default("@ohos:util")),
        ("@kit.ArkTS", "worker") => Some(KitEntry::Default("@ohos:worker")),
        ("@kit.ArkTS", "xml") => Some(KitEntry::Default("@ohos:xml")),

        // ── @kit.ArkUI ───────────────────────────────────────────────
        ("@kit.ArkUI", "AccessibilityOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "AccessibilityOptions",
        )),
        ("@kit.ArkUI", "AccessibilitySelectedType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "AccessibilitySelectedType",
        )),
        ("@kit.ArkUI", "AddFormMenuItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FormMenu",
            "AddFormMenuItem",
        )),
        ("@kit.ArkUI", "AddFormOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FormMenu",
            "AddFormOptions",
        )),
        ("@kit.ArkUI", "AdvancedDialogV2Button") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "AdvancedDialogV2Button",
        )),
        ("@kit.ArkUI", "AdvancedDialogV2ButtonAction") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "AdvancedDialogV2ButtonAction",
        )),
        ("@kit.ArkUI", "AdvancedDialogV2ButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "AdvancedDialogV2ButtonOptions",
        )),
        ("@kit.ArkUI", "AdvancedDialogV2OnCheckedChange") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "AdvancedDialogV2OnCheckedChange",
        )),
        ("@kit.ArkUI", "AlertDialog") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "AlertDialog",
        )),
        ("@kit.ArkUI", "AlertDialogV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "AlertDialogV2",
        )),
        ("@kit.ArkUI", "AlphabetIndexerModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "AlphabetIndexerModifier",
        )),
        ("@kit.ArkUI", "AnimatedDrawableDescriptor") => Some(KitEntry::Named(
            "@ohos:arkui.drawableDescriptor",
            "AnimatedDrawableDescriptor",
        )),
        ("@kit.ArkUI", "AnimationController") => Some(KitEntry::Named(
            "@ohos:arkui.drawableDescriptor",
            "AnimationController",
        )),
        ("@kit.ArkUI", "AnimationOptions") => Some(KitEntry::Named(
            "@ohos:arkui.drawableDescriptor",
            "AnimationOptions",
        )),
        ("@kit.ArkUI", "Animator") => Some(KitEntry::Default("@ohos:animator")),
        ("@kit.ArkUI", "AnimatorOptions") => {
            Some(KitEntry::Named("@ohos:animator", "AnimatorOptions"))
        }
        ("@kit.ArkUI", "AnimatorResult") => {
            Some(KitEntry::Named("@ohos:animator", "AnimatorResult"))
        }
        ("@kit.ArkUI", "App") => Some(KitEntry::Default("@system:app")),
        ("@kit.ArkUI", "AppResponse") => Some(KitEntry::Named("@system:app", "AppResponse")),
        ("@kit.ArkUI", "AppStorageV2") => Some(KitEntry::Named(
            "@ohos:arkui.StateManagement",
            "AppStorageV2",
        )),
        ("@kit.ArkUI", "ArcAlphabetIndexer") => Some(KitEntry::Named(
            "@ohos:arkui.ArcAlphabetIndexer",
            "ArcAlphabetIndexer",
        )),
        ("@kit.ArkUI", "ArcAlphabetIndexerAttribute") => Some(KitEntry::Named(
            "@ohos:arkui.ArcAlphabetIndexer",
            "ArcAlphabetIndexerAttribute",
        )),
        ("@kit.ArkUI", "ArcButton") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcButton",
            "ArcButton",
        )),
        ("@kit.ArkUI", "ArcButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcButton",
            "ArcButtonOptions",
        )),
        ("@kit.ArkUI", "ArcButtonPosition") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcButton",
            "ArcButtonPosition",
        )),
        ("@kit.ArkUI", "ArcButtonStatus") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcButton",
            "ArcButtonStatus",
        )),
        ("@kit.ArkUI", "ArcButtonStyleMode") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcButton",
            "ArcButtonStyleMode",
        )),
        ("@kit.ArkUI", "ArcDirection") => {
            Some(KitEntry::Named("@ohos:arkui.ArcSwiper", "ArcDirection"))
        }
        ("@kit.ArkUI", "ArcDotIndicator") => {
            Some(KitEntry::Named("@ohos:arkui.ArcSwiper", "ArcDotIndicator"))
        }
        ("@kit.ArkUI", "ArcList") => Some(KitEntry::Named("@ohos:arkui.ArcList", "ArcList")),
        ("@kit.ArkUI", "ArcListAttribute") => {
            Some(KitEntry::Named("@ohos:arkui.ArcList", "ArcListAttribute"))
        }
        ("@kit.ArkUI", "ArcListItem") => {
            Some(KitEntry::Named("@ohos:arkui.ArcList", "ArcListItem"))
        }
        ("@kit.ArkUI", "ArcListItemAttribute") => Some(KitEntry::Named(
            "@ohos:arkui.ArcList",
            "ArcListItemAttribute",
        )),
        ("@kit.ArkUI", "ArcScrollBar") => {
            Some(KitEntry::Named("@ohos:arkui.ArcScrollBar", "ArcScrollBar"))
        }
        ("@kit.ArkUI", "ArcScrollBarAttribute") => Some(KitEntry::Named(
            "@ohos:arkui.ArcScrollBar",
            "ArcScrollBarAttribute",
        )),
        ("@kit.ArkUI", "ArcSlider") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSlider",
        )),
        ("@kit.ArkUI", "ArcSliderLayoutOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderLayoutOptions",
        )),
        ("@kit.ArkUI", "ArcSliderLayoutOptionsConstructorOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderLayoutOptionsConstructorOptions",
        )),
        ("@kit.ArkUI", "ArcSliderOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderOptions",
        )),
        ("@kit.ArkUI", "ArcSliderOptionsConstructorOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderOptionsConstructorOptions",
        )),
        ("@kit.ArkUI", "ArcSliderPosition") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderPosition",
        )),
        ("@kit.ArkUI", "ArcSliderStyleOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderStyleOptions",
        )),
        ("@kit.ArkUI", "ArcSliderStyleOptionsConstructorOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderStyleOptionsConstructorOptions",
        )),
        ("@kit.ArkUI", "ArcSliderValueOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderValueOptions",
        )),
        ("@kit.ArkUI", "ArcSliderValueOptionsConstructorOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ArcSlider",
            "ArcSliderValueOptionsConstructorOptions",
        )),
        ("@kit.ArkUI", "ArcSwiper") => Some(KitEntry::Named("@ohos:arkui.ArcSwiper", "ArcSwiper")),
        ("@kit.ArkUI", "ArcSwiperAttribute") => Some(KitEntry::Named(
            "@ohos:arkui.ArcSwiper",
            "ArcSwiperAttribute",
        )),
        ("@kit.ArkUI", "ArcSwiperController") => Some(KitEntry::Named(
            "@ohos:arkui.ArcSwiper",
            "ArcSwiperController",
        )),
        ("@kit.ArkUI", "AtomicServiceBar") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "AtomicServiceBar"))
        }
        ("@kit.ArkUI", "AtomicServiceNavigation") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "AtomicServiceNavigation",
        )),
        ("@kit.ArkUI", "AtomicServiceSearch") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceSearch",
            "AtomicServiceSearch",
        )),
        ("@kit.ArkUI", "AtomicServiceTabs") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceTabs",
            "AtomicServiceTabs",
        )),
        ("@kit.ArkUI", "AtomicServiceWeb") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "AtomicServiceWeb",
        )),
        ("@kit.ArkUI", "AtomicServiceWebController") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "AtomicServiceWebController",
        )),
        ("@kit.ArkUI", "AttributeUpdater") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "AttributeUpdater"))
        }
        ("@kit.ArkUI", "BackRouterOptions") => {
            Some(KitEntry::Named("@system:router", "BackRouterOptions"))
        }
        ("@kit.ArkUI", "BackgroundTheme") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "BackgroundTheme",
        )),
        ("@kit.ArkUI", "BasicPrefetcher") => {
            Some(KitEntry::Named("@ohos:arkui.Prefetcher", "BasicPrefetcher"))
        }
        ("@kit.ArkUI", "Binding") => {
            Some(KitEntry::Named("@ohos:arkui.StateManagement", "Binding"))
        }
        ("@kit.ArkUI", "BlankModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "BlankModifier"))
        }
        ("@kit.ArkUI", "BorderRadiusMode") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "BorderRadiusMode",
        )),
        ("@kit.ArkUI", "BottomOffset") => Some(KitEntry::Named(
            "@ohos:atomicservice.InterstitialDialogAction",
            "BottomOffset",
        )),
        ("@kit.ArkUI", "BuildOptions") => Some(KitEntry::Named("@ohos:arkui.node", "BuildOptions")),
        ("@kit.ArkUI", "BuilderNode") => Some(KitEntry::Named("@ohos:arkui.node", "BuilderNode")),
        ("@kit.ArkUI", "Button") => Some(KitEntry::Named("@system:prompt", "Button")),
        ("@kit.ArkUI", "ButtonModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ButtonModifier"))
        }
        ("@kit.ArkUI", "ButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "ButtonOptions",
        )),
        ("@kit.ArkUI", "CalendarPickerModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "CalendarPickerModifier",
        )),
        ("@kit.ArkUI", "CallbackParam") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TreeView",
            "CallbackParam",
        )),
        ("@kit.ArkUI", "CapsuleSegmentButtonConstructionOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "CapsuleSegmentButtonConstructionOptions",
        )),
        ("@kit.ArkUI", "CapsuleSegmentButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "CapsuleSegmentButtonOptions",
        )),
        ("@kit.ArkUI", "CapsuleSegmentButtonV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "CapsuleSegmentButtonV2",
        )),
        ("@kit.ArkUI", "CheckboxGroupModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "CheckboxGroupModifier",
        )),
        ("@kit.ArkUI", "CheckboxModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "CheckboxModifier"))
        }
        ("@kit.ArkUI", "Chip") => Some(KitEntry::Named("@ohos:arkui.advanced.Chip", "Chip")),
        ("@kit.ArkUI", "ChipGroup") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "ChipGroup",
        )),
        ("@kit.ArkUI", "ChipGroupItemOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "ChipGroupItemOptions",
        )),
        ("@kit.ArkUI", "ChipGroupSpaceOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "ChipGroupSpaceOptions",
        )),
        ("@kit.ArkUI", "ChipItemLabelOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "LabelOptions",
        )),
        ("@kit.ArkUI", "ChipItemStyle") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "ChipItemStyle",
        )),
        ("@kit.ArkUI", "ChipOptions") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.Chip", "ChipOptions"))
        }
        ("@kit.ArkUI", "ChipSize") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.Chip", "ChipSize"))
        }
        ("@kit.ArkUI", "ChipSuffixSymbolGlyphOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "ChipSuffixSymbolGlyphOptions",
        )),
        ("@kit.ArkUI", "ChipSymbolGlyphOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "ChipSymbolGlyphOptions",
        )),
        ("@kit.ArkUI", "CircleShape") => Some(KitEntry::Named("@ohos:arkui.shape", "CircleShape")),
        ("@kit.ArkUI", "CloseOptions") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.Chip", "CloseOptions"))
        }
        ("@kit.ArkUI", "ColorMetrics") => Some(KitEntry::Named("@ohos:arkui.node", "ColorMetrics")),
        ("@kit.ArkUI", "Colors") => Some(KitEntry::Named("@ohos:arkui.theme", "Colors")),
        ("@kit.ArkUI", "ColumnModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ColumnModifier"))
        }
        ("@kit.ArkUI", "ColumnSplitModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "ColumnSplitModifier",
        )),
        ("@kit.ArkUI", "CommonModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "CommonModifier"))
        }
        ("@kit.ArkUI", "CommonSegmentButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "CommonSegmentButtonOptions",
        )),
        ("@kit.ArkUI", "ComponentContent") => {
            Some(KitEntry::Named("@ohos:arkui.node", "ComponentContent"))
        }
        ("@kit.ArkUI", "ComponentUtils") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "ComponentUtils"))
        }
        ("@kit.ArkUI", "ComposeListItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeListItem",
            "ComposeListItem",
        )),
        ("@kit.ArkUI", "ComposeTitleBar") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeTitleBar",
            "ComposeTitleBar",
        )),
        ("@kit.ArkUI", "ComposeTitleBarMenuItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeTitleBar",
            "ComposeTitleBarMenuItem",
        )),
        ("@kit.ArkUI", "Configuration") => Some(KitEntry::Default("@system:configuration")),
        ("@kit.ArkUI", "ConfirmDialog") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "ConfirmDialog",
        )),
        ("@kit.ArkUI", "ConfirmDialogV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "ConfirmDialogV2",
        )),
        ("@kit.ArkUI", "ConnectOptions") => Some(KitEntry::Named(
            "@ohos:arkui.StateManagement",
            "ConnectOptions",
        )),
        ("@kit.ArkUI", "ContainerSpanModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "ContainerSpanModifier",
        )),
        ("@kit.ArkUI", "Content") => Some(KitEntry::Named("@ohos:arkui.node", "Content")),
        ("@kit.ArkUI", "ContentItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeListItem",
            "ContentItem",
        )),
        ("@kit.ArkUI", "ContextMenuController") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "ContextMenuController",
        )),
        ("@kit.ArkUI", "CounterComponent") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Counter",
            "CounterComponent",
        )),
        ("@kit.ArkUI", "CounterModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "CounterModifier"))
        }
        ("@kit.ArkUI", "CounterOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Counter",
            "CounterOptions",
        )),
        ("@kit.ArkUI", "CounterType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Counter",
            "CounterType",
        )),
        ("@kit.ArkUI", "CursorController") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "CursorController"))
        }
        ("@kit.ArkUI", "CustomColors") => {
            Some(KitEntry::Named("@ohos:arkui.theme", "CustomColors"))
        }
        ("@kit.ArkUI", "CustomContentDialog") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "CustomContentDialog",
        )),
        ("@kit.ArkUI", "CustomContentDialogV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "CustomContentDialogV2",
        )),
        ("@kit.ArkUI", "CustomDarkColors") => {
            Some(KitEntry::Named("@ohos:arkui.theme", "CustomDarkColors"))
        }
        ("@kit.ArkUI", "CustomTheme") => Some(KitEntry::Named("@ohos:arkui.theme", "CustomTheme")),
        ("@kit.ArkUI", "DataPanelModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "DataPanelModifier"))
        }
        ("@kit.ArkUI", "DateData") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.Counter", "DateData"))
        }
        ("@kit.ArkUI", "DatePickerModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "DatePickerModifier",
        )),
        ("@kit.ArkUI", "DimensionNoPercentage") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "DimensionNoPercentage",
        )),
        ("@kit.ArkUI", "DisableAlertBeforeBackPageOptions") => Some(KitEntry::Named(
            "@system:router",
            "DisableAlertBeforeBackPageOptions",
        )),
        ("@kit.ArkUI", "DividerModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "DividerModifier"))
        }
        ("@kit.ArkUI", "DownloadContentOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DownloadFileButton",
            "DownloadContentOptions",
        )),
        ("@kit.ArkUI", "DownloadDescription") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DownloadFileButton",
            "DownloadDescription",
        )),
        ("@kit.ArkUI", "DownloadFileButton") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DownloadFileButton",
            "DownloadFileButton",
        )),
        ("@kit.ArkUI", "DownloadIconStyle") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DownloadFileButton",
            "DownloadIconStyle",
        )),
        ("@kit.ArkUI", "DownloadLayoutDirection") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DownloadFileButton",
            "DownloadLayoutDirection",
        )),
        ("@kit.ArkUI", "DownloadStyleOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DownloadFileButton",
            "DownloadStyleOptions",
        )),
        ("@kit.ArkUI", "DragController") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "DragController"))
        }
        ("@kit.ArkUI", "DrawContext") => Some(KitEntry::Named("@ohos:arkui.node", "DrawContext")),
        ("@kit.ArkUI", "DrawableDescriptor") => Some(KitEntry::Named(
            "@ohos:arkui.drawableDescriptor",
            "DrawableDescriptor",
        )),
        ("@kit.ArkUI", "DrawableDescriptorLoadedResult") => Some(KitEntry::Named(
            "@ohos:arkui.drawableDescriptor",
            "DrawableDescriptorLoadedResult",
        )),
        ("@kit.ArkUI", "EditableLeftIconType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.EditableTitleBar",
            "EditableLeftIconType",
        )),
        ("@kit.ArkUI", "EditableTitleBar") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.EditableTitleBar",
            "EditableTitleBar",
        )),
        ("@kit.ArkUI", "EditableTitleBarItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.EditableTitleBar",
            "EditableTitleBarItem",
        )),
        ("@kit.ArkUI", "EditableTitleBarMenuItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.EditableTitleBar",
            "EditableTitleBarMenuItem",
        )),
        ("@kit.ArkUI", "EditableTitleBarOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.EditableTitleBar",
            "EditableTitleBarOptions",
        )),
        ("@kit.ArkUI", "EditorEventInfo") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SelectionMenu",
            "EditorEventInfo",
        )),
        ("@kit.ArkUI", "EditorMenuOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SelectionMenu",
            "EditorMenuOptions",
        )),
        ("@kit.ArkUI", "EllipseShape") => {
            Some(KitEntry::Named("@ohos:arkui.shape", "EllipseShape"))
        }
        ("@kit.ArkUI", "EnableAlertBeforeBackPageOptions") => Some(KitEntry::Named(
            "@system:router",
            "EnableAlertBeforeBackPageOptions",
        )),
        ("@kit.ArkUI", "ExceptionPrompt") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ExceptionPrompt",
            "ExceptionPrompt",
        )),
        ("@kit.ArkUI", "ExpandMode") => Some(KitEntry::Named("@ohos:arkui.node", "ExpandMode")),
        ("@kit.ArkUI", "ExpandedMenuOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SelectionMenu",
            "ExpandedMenuOptions",
        )),
        ("@kit.ArkUI", "ExpandedRegionLayoutOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "ExpandedRegionLayoutOptions",
        )),
        ("@kit.ArkUI", "ExtraRegionPosition") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "ExtraRegionPosition",
        )),
        ("@kit.ArkUI", "Filter") => Some(KitEntry::Named("@ohos:arkui.advanced.Filter", "Filter")),
        ("@kit.ArkUI", "FilterParams") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Filter",
            "FilterParams",
        )),
        ("@kit.ArkUI", "FilterResult") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Filter",
            "FilterResult",
        )),
        ("@kit.ArkUI", "FilterType") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.Filter", "FilterType"))
        }
        ("@kit.ArkUI", "FoldSplitContainer") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "FoldSplitContainer",
        )),
        ("@kit.ArkUI", "FoldedRegionLayoutOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "FoldedRegionLayoutOptions",
        )),
        ("@kit.ArkUI", "Font") => Some(KitEntry::Named("@ohos:arkui.UIContext", "Font")),
        ("@kit.ArkUI", "FormMenuItemStyle") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FormMenu",
            "FormMenuItemStyle",
        )),
        ("@kit.ArkUI", "Frame") => Some(KitEntry::Named("@ohos:arkui.node", "Frame")),
        ("@kit.ArkUI", "FrameCallback") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "FrameCallback"))
        }
        ("@kit.ArkUI", "FrameNode") => Some(KitEntry::Named("@ohos:arkui.node", "FrameNode")),
        ("@kit.ArkUI", "FullScreenLaunchComponent") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FullScreenLaunchComponent",
            "FullScreenLaunchComponent",
        )),
        ("@kit.ArkUI", "GaugeModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "GaugeModifier"))
        }
        ("@kit.ArkUI", "GestureActionPhase") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "GestureActionPhase",
        )),
        ("@kit.ArkUI", "GestureListenerCallback") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "GestureListenerCallback",
        )),
        ("@kit.ArkUI", "GestureListenerType") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "GestureListenerType",
        )),
        ("@kit.ArkUI", "GestureObserverConfigs") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "GestureObserverConfigs",
        )),
        ("@kit.ArkUI", "GestureTriggerInfo") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "GestureTriggerInfo",
        )),
        ("@kit.ArkUI", "GradientAlpha") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "GradientAlpha",
        )),
        ("@kit.ArkUI", "GradientBackground") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "GradientBackground",
        )),
        ("@kit.ArkUI", "GridColModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "GridColModifier"))
        }
        ("@kit.ArkUI", "GridItemModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "GridItemModifier"))
        }
        ("@kit.ArkUI", "GridModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "GridModifier"))
        }
        ("@kit.ArkUI", "GridObjectSortComponent") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.GridObjectSortComponent",
            "GridObjectSortComponent",
        )),
        ("@kit.ArkUI", "GridObjectSortComponentItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.GridObjectSortComponent",
            "GridObjectSortComponentItem",
        )),
        ("@kit.ArkUI", "GridObjectSortComponentOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.GridObjectSortComponent",
            "GridObjectSortComponentOptions",
        )),
        ("@kit.ArkUI", "GridObjectSortComponentType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.GridObjectSortComponent",
            "GridObjectSortComponentType",
        )),
        ("@kit.ArkUI", "GridRowModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "GridRowModifier"))
        }
        ("@kit.ArkUI", "HalfScreenLaunchComponent") => Some(KitEntry::Named(
            "@ohos:atomicservice.HalfScreenLaunchComponent",
            "HalfScreenLaunchComponent",
        )),
        ("@kit.ArkUI", "HoverModeRegionLayoutOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "HoverModeRegionLayoutOptions",
        )),
        ("@kit.ArkUI", "HoverModeStatus") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "HoverModeStatus",
        )),
        ("@kit.ArkUI", "HyperlinkModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "HyperlinkModifier"))
        }
        ("@kit.ArkUI", "IDataSourcePrefetching") => Some(KitEntry::Named(
            "@ohos:arkui.Prefetcher",
            "IDataSourcePrefetching",
        )),
        ("@kit.ArkUI", "IPrefetcher") => {
            Some(KitEntry::Named("@ohos:arkui.Prefetcher", "IPrefetcher"))
        }
        ("@kit.ArkUI", "IconCommonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "IconCommonOptions",
        )),
        ("@kit.ArkUI", "IconGroupSuffix") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "IconGroupSuffix",
        )),
        ("@kit.ArkUI", "IconItemOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "IconItemOptions",
        )),
        ("@kit.ArkUI", "IconOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "IconOptions",
        )),
        ("@kit.ArkUI", "IconStyle") => Some(KitEntry::Named(
            "@ohos:atomicservice.InterstitialDialogAction",
            "IconStyle",
        )),
        ("@kit.ArkUI", "IconType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeListItem",
            "IconType",
        )),
        ("@kit.ArkUI", "ImageAnimatorModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "ImageAnimatorModifier",
        )),
        ("@kit.ArkUI", "ImageModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ImageModifier"))
        }
        ("@kit.ArkUI", "ImageSpanModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ImageSpanModifier"))
        }
        ("@kit.ArkUI", "ImmersiveMode") => {
            Some(KitEntry::Named("@ohos:promptAction", "ImmersiveMode"))
        }
        ("@kit.ArkUI", "InputEventType") => {
            Some(KitEntry::Named("@ohos:arkui.node", "InputEventType"))
        }
        ("@kit.ArkUI", "InputFilterParams") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceSearch",
            "InputFilterParams",
        )),
        ("@kit.ArkUI", "InterstitialDialogAction") => Some(KitEntry::Named(
            "@ohos:atomicservice.InterstitialDialogAction",
            "InterstitialDialogAction",
        )),
        ("@kit.ArkUI", "ItemRestriction") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "ItemRestriction",
        )),
        ("@kit.ArkUI", "ItemState") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.ToolBar", "ItemState"))
        }
        ("@kit.ArkUI", "KeyboardAvoidMode") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "KeyboardAvoidMode",
        )),
        ("@kit.ArkUI", "LabelMarginOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "LabelMarginOptions",
        )),
        ("@kit.ArkUI", "LabelOptions") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.Chip", "LabelOptions"))
        }
        ("@kit.ArkUI", "LayeredDrawableDescriptor") => Some(KitEntry::Named(
            "@ohos:arkui.drawableDescriptor",
            "LayeredDrawableDescriptor",
        )),
        ("@kit.ArkUI", "LayoutConstraint") => {
            Some(KitEntry::Named("@ohos:arkui.node", "LayoutConstraint"))
        }
        ("@kit.ArkUI", "LengthMetrics") => {
            Some(KitEntry::Named("@ohos:arkui.node", "LengthMetrics"))
        }
        ("@kit.ArkUI", "LengthMetricsUnit") => {
            Some(KitEntry::Named("@ohos:arkui.node", "LengthMetricsUnit"))
        }
        ("@kit.ArkUI", "LengthUnit") => Some(KitEntry::Named("@ohos:arkui.node", "LengthUnit")),
        ("@kit.ArkUI", "LevelMode") => Some(KitEntry::Named("@ohos:promptAction", "LevelMode")),
        ("@kit.ArkUI", "LevelOrder") => Some(KitEntry::Named("@ohos:promptAction", "LevelOrder")),
        ("@kit.ArkUI", "LineModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "LineModifier"))
        }
        ("@kit.ArkUI", "ListItemGroupModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "ListItemGroupModifier",
        )),
        ("@kit.ArkUI", "ListItemModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ListItemModifier"))
        }
        ("@kit.ArkUI", "ListModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ListModifier"))
        }
        ("@kit.ArkUI", "LoadingDialog") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "LoadingDialog",
        )),
        ("@kit.ArkUI", "LoadingDialogV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "LoadingDialogV2",
        )),
        ("@kit.ArkUI", "LoadingProgressModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "LoadingProgressModifier",
        )),
        ("@kit.ArkUI", "LocaleResponse") => {
            Some(KitEntry::Named("@system:configuration", "LocaleResponse"))
        }
        ("@kit.ArkUI", "MarginType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ExceptionPrompt",
            "MarginType",
        )),
        ("@kit.ArkUI", "MarqueeDynamicSyncScene") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "MarqueeDynamicSyncScene",
        )),
        ("@kit.ArkUI", "MarqueeDynamicSyncSceneType") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "MarqueeDynamicSyncSceneType",
        )),
        ("@kit.ArkUI", "MarqueeModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "MarqueeModifier"))
        }
        ("@kit.ArkUI", "Matrix4") => Some(KitEntry::Named("@ohos:arkui.node", "Matrix4")),
        ("@kit.ArkUI", "MeasureOptions") => {
            Some(KitEntry::Named("@ohos:measure", "MeasureOptions"))
        }
        ("@kit.ArkUI", "MeasureText") => Some(KitEntry::Default("@ohos:measure")),
        ("@kit.ArkUI", "MeasureUtils") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "MeasureUtils"))
        }
        ("@kit.ArkUI", "MediaQuery") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "MediaQuery"))
        }
        ("@kit.ArkUI", "MediaQueryEvent") => {
            Some(KitEntry::Named("@system:mediaquery", "MediaQueryEvent"))
        }
        ("@kit.ArkUI", "MediaQueryList") => {
            Some(KitEntry::Named("@system:mediaquery", "MediaQueryList"))
        }
        ("@kit.ArkUI", "MenuAlignParams") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceSearch",
            "MenuAlignParams",
        )),
        ("@kit.ArkUI", "MenuItemModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "MenuItemModifier"))
        }
        ("@kit.ArkUI", "MenuModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "MenuModifier"))
        }
        ("@kit.ArkUI", "MixMode") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "MixMode",
        )),
        ("@kit.ArkUI", "MultiCapsuleSegmentButtonV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "MultiCapsuleSegmentButtonV2",
        )),
        ("@kit.ArkUI", "MultiNavPathStack") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.MultiNavigation",
            "MultiNavPathStack",
        )),
        ("@kit.ArkUI", "MultiNavigation") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.MultiNavigation",
            "MultiNavigation",
        )),
        ("@kit.ArkUI", "MutableBinding") => Some(KitEntry::Named(
            "@ohos:arkui.StateManagement",
            "MutableBinding",
        )),
        ("@kit.ArkUI", "NavDestinationBuilder") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "NavDestinationBuilder",
        )),
        ("@kit.ArkUI", "NavDestinationModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "NavDestinationModifier",
        )),
        ("@kit.ArkUI", "NavPushPathHelper") => Some(KitEntry::Named(
            "@ohos:atomicservice.NavPushPathHelper",
            "NavPushPathHelper",
        )),
        ("@kit.ArkUI", "NavRouterModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "NavRouterModifier"))
        }
        ("@kit.ArkUI", "NavigationModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "NavigationModifier",
        )),
        ("@kit.ArkUI", "NavigatorModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "NavigatorModifier"))
        }
        ("@kit.ArkUI", "NodeAdapter") => Some(KitEntry::Named("@ohos:arkui.node", "NodeAdapter")),
        ("@kit.ArkUI", "NodeContent") => Some(KitEntry::Named("@ohos:arkui.node", "NodeContent")),
        ("@kit.ArkUI", "NodeController") => {
            Some(KitEntry::Named("@ohos:arkui.node", "NodeController"))
        }
        ("@kit.ArkUI", "NodeIdentity") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "NodeIdentity"))
        }
        ("@kit.ArkUI", "NodeParam") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TreeView",
            "NodeParam",
        )),
        ("@kit.ArkUI", "NodeRenderState") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "NodeRenderState"))
        }
        ("@kit.ArkUI", "NodeRenderStateChangeCallback") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "NodeRenderStateChangeCallback",
        )),
        ("@kit.ArkUI", "NodeRenderType") => {
            Some(KitEntry::Named("@ohos:arkui.node", "NodeRenderType"))
        }
        ("@kit.ArkUI", "Offset") => Some(KitEntry::Named("@ohos:arkui.node", "Offset")),
        ("@kit.ArkUI", "OnContentWillChangeCallback") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceTabs",
            "OnContentWillChangeCallback",
        )),
        ("@kit.ArkUI", "OnErrorReceiveEvent") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnErrorReceiveEvent",
        )),
        ("@kit.ArkUI", "OnHoverStatusChangeHandler") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "OnHoverStatusChangeHandler",
        )),
        ("@kit.ArkUI", "OnHttpErrorReceiveEvent") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnHttpErrorReceiveEvent",
        )),
        ("@kit.ArkUI", "OnLoadInterceptCallback") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnLoadInterceptCallback",
        )),
        ("@kit.ArkUI", "OnLoadInterceptEvent") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnLoadInterceptEvent",
        )),
        ("@kit.ArkUI", "OnMessageEvent") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnMessageEvent",
        )),
        ("@kit.ArkUI", "OnPageBeginEvent") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnPageBeginEvent",
        )),
        ("@kit.ArkUI", "OnPageEndEvent") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnPageEndEvent",
        )),
        ("@kit.ArkUI", "OnProgressChangeEvent") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "OnProgressChangeEvent",
        )),
        ("@kit.ArkUI", "OnSelectedIndexChange") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "OnSelectedIndexChange",
        )),
        ("@kit.ArkUI", "OnSelectedIndexesChange") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "OnSelectedIndexesChange",
        )),
        ("@kit.ArkUI", "OperateButton") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeListItem",
            "OperateButton",
        )),
        ("@kit.ArkUI", "OperateCheck") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeListItem",
            "OperateCheck",
        )),
        ("@kit.ArkUI", "OperateIcon") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeListItem",
            "OperateIcon",
        )),
        ("@kit.ArkUI", "OperateItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ComposeListItem",
            "OperateItem",
        )),
        ("@kit.ArkUI", "OperationOption") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeader",
            "OperationOption",
        )),
        ("@kit.ArkUI", "OperationParams") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceSearch",
            "OperationParams",
        )),
        ("@kit.ArkUI", "OperationType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeader",
            "OperationType",
        )),
        ("@kit.ArkUI", "OverlayManager") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "OverlayManager"))
        }
        ("@kit.ArkUI", "OverlayManagerOptions") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "OverlayManagerOptions",
        )),
        ("@kit.ArkUI", "PageInfo") => Some(KitEntry::Named("@ohos:arkui.UIContext", "PageInfo")),
        ("@kit.ArkUI", "PanelModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "PanelModifier"))
        }
        ("@kit.ArkUI", "ParticleModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ParticleModifier"))
        }
        ("@kit.ArkUI", "PathModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "PathModifier"))
        }
        ("@kit.ArkUI", "PathShape") => Some(KitEntry::Named("@ohos:arkui.shape", "PathShape")),
        ("@kit.ArkUI", "PatternLockModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "PatternLockModifier",
        )),
        ("@kit.ArkUI", "PersistenceV2") => Some(KitEntry::Named(
            "@ohos:arkui.StateManagement",
            "PersistenceV2",
        )),
        ("@kit.ArkUI", "PiPWindow") => Some(KitEntry::Default("@ohos:PiPWindow")),
        ("@kit.ArkUI", "Pivot") => Some(KitEntry::Named("@ohos:arkui.node", "Pivot")),
        ("@kit.ArkUI", "PixelMapDrawableDescriptor") => Some(KitEntry::Named(
            "@ohos:arkui.drawableDescriptor",
            "PixelMapDrawableDescriptor",
        )),
        ("@kit.ArkUI", "PluginComponentTemplate") => Some(KitEntry::Named(
            "@ohos:pluginComponent",
            "PluginComponentTemplate",
        )),
        ("@kit.ArkUI", "PolygonModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "PolygonModifier"))
        }
        ("@kit.ArkUI", "PolylineModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "PolylineModifier"))
        }
        ("@kit.ArkUI", "PopoverDialog") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "PopoverDialog",
        )),
        ("@kit.ArkUI", "PopoverDialogV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "PopoverDialogV2",
        )),
        ("@kit.ArkUI", "PopoverDialogV2OnVisibleChange") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "PopoverDialogV2OnVisibleChange",
        )),
        ("@kit.ArkUI", "PopoverDialogV2Options") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "PopoverDialogV2Options",
        )),
        ("@kit.ArkUI", "PopoverOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "PopoverOptions",
        )),
        ("@kit.ArkUI", "Popup") => Some(KitEntry::Named("@ohos:arkui.advanced.Popup", "Popup")),
        ("@kit.ArkUI", "PopupButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Popup",
            "PopupButtonOptions",
        )),
        ("@kit.ArkUI", "PopupIconOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Popup",
            "PopupIconOptions",
        )),
        ("@kit.ArkUI", "PopupOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Popup",
            "PopupOptions",
        )),
        ("@kit.ArkUI", "PopupTextOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Popup",
            "PopupTextOptions",
        )),
        ("@kit.ArkUI", "Position") => Some(KitEntry::Named("@ohos:arkui.node", "Position")),
        ("@kit.ArkUI", "PrefixIconOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "PrefixIconOptions",
        )),
        ("@kit.ArkUI", "PresetSplitRatio") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.FoldSplitContainer",
            "PresetSplitRatio",
        )),
        ("@kit.ArkUI", "ProgressButton") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ProgressButton",
            "ProgressButton",
        )),
        ("@kit.ArkUI", "ProgressButtonV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ProgressButtonV2",
            "ProgressButtonV2",
        )),
        ("@kit.ArkUI", "ProgressButtonV2Color") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ProgressButtonV2",
            "ProgressButtonV2Color",
        )),
        ("@kit.ArkUI", "ProgressButtonV2ColorOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ProgressButtonV2",
            "ProgressButtonV2ColorOptions",
        )),
        ("@kit.ArkUI", "ProgressModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ProgressModifier"))
        }
        ("@kit.ArkUI", "Prompt") => Some(KitEntry::Default("@system:prompt")),
        ("@kit.ArkUI", "PromptAction") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "PromptAction"))
        }
        ("@kit.ArkUI", "PromptOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ExceptionPrompt",
            "PromptOptions",
        )),
        ("@kit.ArkUI", "QRCodeModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "QRCodeModifier"))
        }
        ("@kit.ArkUI", "RadioModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "RadioModifier"))
        }
        ("@kit.ArkUI", "RatingModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "RatingModifier"))
        }
        ("@kit.ArkUI", "Rect") => Some(KitEntry::Named("@ohos:arkui.node", "Rect")),
        ("@kit.ArkUI", "RectModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "RectModifier"))
        }
        ("@kit.ArkUI", "RectShape") => Some(KitEntry::Named("@ohos:arkui.shape", "RectShape")),
        ("@kit.ArkUI", "RefreshModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "RefreshModifier"))
        }
        ("@kit.ArkUI", "RenderNode") => Some(KitEntry::Named("@ohos:arkui.node", "RenderNode")),
        ("@kit.ArkUI", "RenderOptions") => {
            Some(KitEntry::Named("@ohos:arkui.node", "RenderOptions"))
        }
        ("@kit.ArkUI", "RequestFullWindowOptions") => {
            Some(KitEntry::Named("@system:app", "RequestFullWindowOptions"))
        }
        ("@kit.ArkUI", "RichEditorModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "RichEditorModifier",
        )),
        ("@kit.ArkUI", "Rotation") => Some(KitEntry::Named("@ohos:arkui.node", "Rotation")),
        ("@kit.ArkUI", "RoundRect") => Some(KitEntry::Named("@ohos:arkui.node", "RoundRect")),
        ("@kit.ArkUI", "Router") => Some(KitEntry::Named("@ohos:arkui.UIContext", "Router")),
        ("@kit.ArkUI", "RouterOptions") => Some(KitEntry::Named("@system:router", "RouterOptions")),
        ("@kit.ArkUI", "RouterState") => Some(KitEntry::Named("@system:router", "RouterState")),
        ("@kit.ArkUI", "RowModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "RowModifier"))
        }
        ("@kit.ArkUI", "RowSplitModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "RowSplitModifier"))
        }
        ("@kit.ArkUI", "Scale") => Some(KitEntry::Named("@ohos:arkui.node", "Scale")),
        ("@kit.ArkUI", "ScreenOnVisibleOptions") => {
            Some(KitEntry::Named("@system:app", "ScreenOnVisibleOptions"))
        }
        ("@kit.ArkUI", "ScrollModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ScrollModifier"))
        }
        ("@kit.ArkUI", "SearchButtonParams") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceSearch",
            "SearchButtonParams",
        )),
        ("@kit.ArkUI", "SearchModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "SearchModifier"))
        }
        ("@kit.ArkUI", "SearchParams") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceSearch",
            "SearchParams",
        )),
        ("@kit.ArkUI", "SegmentButton") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButton",
        )),
        ("@kit.ArkUI", "SegmentButtonIconItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonIconItem",
        )),
        ("@kit.ArkUI", "SegmentButtonIconTextItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonIconTextItem",
        )),
        ("@kit.ArkUI", "SegmentButtonItemArray") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonItemArray",
        )),
        ("@kit.ArkUI", "SegmentButtonItemOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonItemOptions",
        )),
        ("@kit.ArkUI", "SegmentButtonItemOptionsArray") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonItemOptionsArray",
        )),
        ("@kit.ArkUI", "SegmentButtonItemOptionsConstructorOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonItemOptionsConstructorOptions",
        )),
        ("@kit.ArkUI", "SegmentButtonItemTuple") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonItemTuple",
        )),
        ("@kit.ArkUI", "SegmentButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonOptions",
        )),
        ("@kit.ArkUI", "SegmentButtonTextItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "SegmentButtonTextItem",
        )),
        ("@kit.ArkUI", "SegmentButtonV2Item") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "SegmentButtonV2Item",
        )),
        ("@kit.ArkUI", "SegmentButtonV2ItemOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "SegmentButtonV2ItemOptions",
        )),
        ("@kit.ArkUI", "SegmentButtonV2Items") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "SegmentButtonV2Items",
        )),
        ("@kit.ArkUI", "SelectDialog") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Dialog",
            "SelectDialog",
        )),
        ("@kit.ArkUI", "SelectDialogV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "SelectDialogV2",
        )),
        ("@kit.ArkUI", "SelectModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "SelectModifier"))
        }
        ("@kit.ArkUI", "SelectOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeader",
            "SelectOptions",
        )),
        ("@kit.ArkUI", "SelectParams") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceSearch",
            "SelectParams",
        )),
        ("@kit.ArkUI", "SelectTitleBar") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SelectTitleBar",
            "SelectTitleBar",
        )),
        ("@kit.ArkUI", "SelectTitleBarMenuItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SelectTitleBar",
            "SelectTitleBarMenuItem",
        )),
        ("@kit.ArkUI", "SelectionMenu") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SelectionMenu",
            "SelectionMenu",
        )),
        ("@kit.ArkUI", "SelectionMenuOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SelectionMenu",
            "SelectionMenuOptions",
        )),
        ("@kit.ArkUI", "ShapeClip") => Some(KitEntry::Named("@ohos:arkui.node", "ShapeClip")),
        ("@kit.ArkUI", "ShapeMask") => Some(KitEntry::Named("@ohos:arkui.node", "ShapeMask")),
        ("@kit.ArkUI", "ShapeModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ShapeModifier"))
        }
        ("@kit.ArkUI", "ShowActionMenuOptions") => {
            Some(KitEntry::Named("@system:prompt", "ShowActionMenuOptions"))
        }
        ("@kit.ArkUI", "ShowDialogOptions") => {
            Some(KitEntry::Named("@system:prompt", "ShowDialogOptions"))
        }
        ("@kit.ArkUI", "ShowDialogSuccessResponse") => Some(KitEntry::Named(
            "@system:prompt",
            "ShowDialogSuccessResponse",
        )),
        ("@kit.ArkUI", "ShowToastOptions") => {
            Some(KitEntry::Named("@system:prompt", "ShowToastOptions"))
        }
        ("@kit.ArkUI", "SideBarContainerModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "SideBarContainerModifier",
        )),
        ("@kit.ArkUI", "SideBarOptions") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "SideBarOptions",
        )),
        ("@kit.ArkUI", "SimpleAnimatorOptions") => {
            Some(KitEntry::Named("@ohos:animator", "SimpleAnimatorOptions"))
        }
        ("@kit.ArkUI", "Size") => Some(KitEntry::Named("@ohos:arkui.node", "Size")),
        ("@kit.ArkUI", "SliderModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "SliderModifier"))
        }
        ("@kit.ArkUI", "SpanModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "SpanModifier"))
        }
        ("@kit.ArkUI", "SplitLayout") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SplitLayout",
            "SplitLayout",
        )),
        ("@kit.ArkUI", "SplitPolicy") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.MultiNavigation",
            "SplitPolicy",
        )),
        ("@kit.ArkUI", "StackModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "StackModifier"))
        }
        ("@kit.ArkUI", "StepperItemModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "StepperItemModifier",
        )),
        ("@kit.ArkUI", "StepperModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "StepperModifier"))
        }
        ("@kit.ArkUI", "SubHeader") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeader",
            "SubHeader",
        )),
        ("@kit.ArkUI", "SubHeaderV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeaderV2",
            "SubHeaderV2",
        )),
        ("@kit.ArkUI", "SubHeaderV2IconType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeaderV2",
            "SubHeaderV2IconType",
        )),
        ("@kit.ArkUI", "SubHeaderV2OperationItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeaderV2",
            "SubHeaderV2OperationItem",
        )),
        ("@kit.ArkUI", "SubHeaderV2OperationItemType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeaderV2",
            "SubHeaderV2OperationItemType",
        )),
        ("@kit.ArkUI", "SubHeaderV2OperationType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeaderV2",
            "SubHeaderV2OperationType",
        )),
        ("@kit.ArkUI", "SubHeaderV2Select") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeaderV2",
            "SubHeaderV2Select",
        )),
        ("@kit.ArkUI", "SubHeaderV2Title") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeaderV2",
            "SubHeaderV2Title",
        )),
        ("@kit.ArkUI", "SuffixIconOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.Chip",
            "SuffixIconOptions",
        )),
        ("@kit.ArkUI", "SuffixImageIconOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "SuffixImageIconOptions",
        )),
        ("@kit.ArkUI", "SwipeRefresher") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SwipeRefresher",
            "SwipeRefresher",
        )),
        ("@kit.ArkUI", "SwiperDynamicSyncScene") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "SwiperDynamicSyncScene",
        )),
        ("@kit.ArkUI", "SwiperDynamicSyncSceneType") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "SwiperDynamicSyncSceneType",
        )),
        ("@kit.ArkUI", "SwiperModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "SwiperModifier"))
        }
        ("@kit.ArkUI", "SymbolGlyphModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "SymbolGlyphModifier",
        )),
        ("@kit.ArkUI", "SymbolItemOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ChipGroup",
            "SymbolItemOptions",
        )),
        ("@kit.ArkUI", "SymbolOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SubHeader",
            "SymbolOptions",
        )),
        ("@kit.ArkUI", "SymbolSpanModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "SymbolSpanModifier",
        )),
        ("@kit.ArkUI", "SystemMediaQuery") => Some(KitEntry::Default("@system:mediaquery")),
        ("@kit.ArkUI", "SystemRouter") => Some(KitEntry::Default("@system:router")),
        ("@kit.ArkUI", "TabBarOptions") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceTabs",
            "TabBarOptions",
        )),
        ("@kit.ArkUI", "TabBarPosition") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceTabs",
            "TabBarPosition",
        )),
        ("@kit.ArkUI", "TabContentBuilder") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceTabs",
            "TabContentBuilder",
        )),
        ("@kit.ArkUI", "TabSegmentButtonConstructionOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "TabSegmentButtonConstructionOptions",
        )),
        ("@kit.ArkUI", "TabSegmentButtonOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButton",
            "TabSegmentButtonOptions",
        )),
        ("@kit.ArkUI", "TabSegmentButtonV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.SegmentButtonV2",
            "TabSegmentButtonV2",
        )),
        ("@kit.ArkUI", "TabTitleBar") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TabTitleBar",
            "TabTitleBar",
        )),
        ("@kit.ArkUI", "TabTitleBarMenuItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TabTitleBar",
            "TabTitleBarMenuItem",
        )),
        ("@kit.ArkUI", "TabTitleBarTabItem") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TabTitleBar",
            "TabTitleBarTabItem",
        )),
        ("@kit.ArkUI", "TabsModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "TabsModifier"))
        }
        ("@kit.ArkUI", "TargetInfo") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "TargetInfo"))
        }
        ("@kit.ArkUI", "TextAreaModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "TextAreaModifier"))
        }
        ("@kit.ArkUI", "TextClockModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "TextClockModifier"))
        }
        ("@kit.ArkUI", "TextInputModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "TextInputModifier"))
        }
        ("@kit.ArkUI", "TextMenuController") => Some(KitEntry::Named(
            "@ohos:arkui.UIContext",
            "TextMenuController",
        )),
        ("@kit.ArkUI", "TextModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "TextModifier"))
        }
        ("@kit.ArkUI", "TextPickerModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "TextPickerModifier",
        )),
        ("@kit.ArkUI", "TextTimerModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "TextTimerModifier"))
        }
        ("@kit.ArkUI", "Theme") => Some(KitEntry::Named("@ohos:arkui.theme", "Theme")),
        ("@kit.ArkUI", "ThemeControl") => {
            Some(KitEntry::Named("@ohos:arkui.theme", "ThemeControl"))
        }
        ("@kit.ArkUI", "TimePickerModifier") => Some(KitEntry::Named(
            "@ohos:arkui.modifier",
            "TimePickerModifier",
        )),
        ("@kit.ArkUI", "TipsDialog") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.Dialog", "TipsDialog"))
        }
        ("@kit.ArkUI", "TipsDialogV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.DialogV2",
            "TipsDialogV2",
        )),
        ("@kit.ArkUI", "TitleBarType") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "TitleBarType",
        )),
        ("@kit.ArkUI", "TitleOptions") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceNavigation",
            "TitleOptions",
        )),
        ("@kit.ArkUI", "TitlePosition") => Some(KitEntry::Named(
            "@ohos:atomicservice.InterstitialDialogAction",
            "TitlePosition",
        )),
        ("@kit.ArkUI", "ToggleModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "ToggleModifier"))
        }
        ("@kit.ArkUI", "ToolBar") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.ToolBar", "ToolBar"))
        }
        ("@kit.ArkUI", "ToolBarModifier") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBar",
            "ToolBarModifier",
        )),
        ("@kit.ArkUI", "ToolBarOption") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBar",
            "ToolBarOption",
        )),
        ("@kit.ArkUI", "ToolBarOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBar",
            "ToolBarOptions",
        )),
        ("@kit.ArkUI", "ToolBarV2") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2",
        )),
        ("@kit.ArkUI", "ToolBarV2Item") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2Item",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemAction") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemAction",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemIconType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemIconType",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemImage") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemImage",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemImageOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemImageOptions",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemOptions",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemState") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemState",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemText") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemText",
        )),
        ("@kit.ArkUI", "ToolBarV2ItemTextOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2ItemTextOptions",
        )),
        ("@kit.ArkUI", "ToolBarV2Modifier") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2Modifier",
        )),
        ("@kit.ArkUI", "ToolBarV2SymbolGlyph") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2SymbolGlyph",
        )),
        ("@kit.ArkUI", "ToolBarV2SymbolGlyphOptions") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.ToolBarV2",
            "ToolBarV2SymbolGlyphOptions",
        )),
        ("@kit.ArkUI", "Translation") => Some(KitEntry::Named("@ohos:arkui.node", "Translation")),
        ("@kit.ArkUI", "TreeController") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TreeView",
            "TreeController",
        )),
        ("@kit.ArkUI", "TreeListenType") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TreeView",
            "TreeListenType",
        )),
        ("@kit.ArkUI", "TreeListener") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TreeView",
            "TreeListener",
        )),
        ("@kit.ArkUI", "TreeListenerManager") => Some(KitEntry::Named(
            "@ohos:arkui.advanced.TreeView",
            "TreeListenerManager",
        )),
        ("@kit.ArkUI", "TreeView") => {
            Some(KitEntry::Named("@ohos:arkui.advanced.TreeView", "TreeView"))
        }
        ("@kit.ArkUI", "Type") => Some(KitEntry::Named("@ohos:arkui.StateManagement", "Type")),
        ("@kit.ArkUI", "UIContext") => Some(KitEntry::Named("@ohos:arkui.UIContext", "UIContext")),
        ("@kit.ArkUI", "UIInspector") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "UIInspector"))
        }
        ("@kit.ArkUI", "UIObserver") => {
            Some(KitEntry::Named("@ohos:arkui.UIContext", "UIObserver"))
        }
        ("@kit.ArkUI", "UIState") => Some(KitEntry::Named("@ohos:arkui.node", "UIState")),
        ("@kit.ArkUI", "UIUtils") => {
            Some(KitEntry::Named("@ohos:arkui.StateManagement", "UIUtils"))
        }
        ("@kit.ArkUI", "VideoModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "VideoModifier"))
        }
        ("@kit.ArkUI", "WaterFlowModifier") => {
            Some(KitEntry::Named("@ohos:arkui.modifier", "WaterFlowModifier"))
        }
        ("@kit.ArkUI", "WebHeader") => Some(KitEntry::Named(
            "@ohos:atomicservice.AtomicServiceWeb",
            "WebHeader",
        )),
        ("@kit.ArkUI", "XComponentNode") => {
            Some(KitEntry::Named("@ohos:arkui.node", "XComponentNode"))
        }
        ("@kit.ArkUI", "borderRadiuses") => {
            Some(KitEntry::Named("@ohos:arkui.node", "borderRadiuses"))
        }
        ("@kit.ArkUI", "borderStyles") => Some(KitEntry::Named("@ohos:arkui.node", "borderStyles")),
        ("@kit.ArkUI", "componentSnapshot") => {
            Some(KitEntry::Default("@ohos:arkui.componentSnapshot"))
        }
        ("@kit.ArkUI", "componentUtils") => Some(KitEntry::Default("@ohos:arkui.componentUtils")),
        ("@kit.ArkUI", "curves") => Some(KitEntry::Default("@ohos:curves")),
        ("@kit.ArkUI", "display") => Some(KitEntry::Default("@ohos:display")),
        ("@kit.ArkUI", "dragController") => Some(KitEntry::Default("@ohos:arkui.dragController")),
        ("@kit.ArkUI", "edgeColors") => Some(KitEntry::Named("@ohos:arkui.node", "edgeColors")),
        ("@kit.ArkUI", "edgeWidths") => Some(KitEntry::Named("@ohos:arkui.node", "edgeWidths")),
        ("@kit.ArkUI", "floatingBall") => Some(KitEntry::Default("@ohos:window.floatingBall")),
        ("@kit.ArkUI", "font") => Some(KitEntry::Default("@ohos:font")),
        ("@kit.ArkUI", "inspector") => Some(KitEntry::Default("@ohos:arkui.inspector")),
        ("@kit.ArkUI", "matrix4") => Some(KitEntry::Default("@ohos:matrix4")),
        ("@kit.ArkUI", "mediaquery") => Some(KitEntry::Default("@ohos:mediaquery")),
        ("@kit.ArkUI", "pluginComponentManager") => {
            Some(KitEntry::Default("@ohos:pluginComponent"))
        }
        ("@kit.ArkUI", "prompt") => Some(KitEntry::Default("@ohos:prompt")),
        ("@kit.ArkUI", "promptAction") => Some(KitEntry::Default("@ohos:promptAction")),
        ("@kit.ArkUI", "router") => Some(KitEntry::Default("@ohos:router")),
        ("@kit.ArkUI", "screenshot") => Some(KitEntry::Default("@ohos:screenshot")),
        ("@kit.ArkUI", "typeNode") => Some(KitEntry::Named("@ohos:arkui.node", "typeNode")),
        ("@kit.ArkUI", "uiAppearance") => Some(KitEntry::Default("@ohos:uiAppearance")),
        ("@kit.ArkUI", "uiExtension") => Some(KitEntry::Default("@ohos:arkui.uiExtension")),
        ("@kit.ArkUI", "uiObserver") => Some(KitEntry::Default("@ohos:arkui.observer")),
        ("@kit.ArkUI", "window") => Some(KitEntry::Default("@ohos:window")),

        // ── @kit.ArkWeb ───────────────────────────────────────────────
        ("@kit.ArkWeb", "ConnectionInfo") => Some(KitEntry::Named(
            "@ohos:web.WebNativeMessagingExtensionAbility",
            "ConnectionInfo",
        )),
        ("@kit.ArkWeb", "WebNativeMessagingExtensionAbility") => Some(KitEntry::Default(
            "@ohos:web.WebNativeMessagingExtensionAbility",
        )),
        ("@kit.ArkWeb", "WebNetErrorList") => {
            Some(KitEntry::Named("@ohos:web.netErrorList", "WebNetErrorList"))
        }
        ("@kit.ArkWeb", "webNativeMessagingExtensionManager") => Some(KitEntry::Default(
            "@ohos:web.webNativeMessagingExtensionManager",
        )),
        ("@kit.ArkWeb", "webview") => Some(KitEntry::Default("@ohos:web.webview")),

        // ── @kit.AssetStoreKit ───────────────────────────────────────────────
        ("@kit.AssetStoreKit", "asset") => Some(KitEntry::Default("@ohos:security.asset")),

        // ── @kit.AudioKit ───────────────────────────────────────────────
        ("@kit.AudioKit", "AVVolumePanel") => Some(KitEntry::Named(
            "@ohos:multimedia.avVolumePanel",
            "AVVolumePanel",
        )),
        ("@kit.AudioKit", "AVVolumePanelParameter") => Some(KitEntry::Named(
            "@ohos:multimedia.avVolumePanel",
            "AVVolumePanelParameter",
        )),
        ("@kit.AudioKit", "audio") => Some(KitEntry::Default("@ohos:multimedia.audio")),
        ("@kit.AudioKit", "audioHaptic") => Some(KitEntry::Default("@ohos:multimedia.audioHaptic")),

        // ── @kit.BackgroundTasksKit ───────────────────────────────────────────────
        ("@kit.BackgroundTasksKit", "WorkSchedulerExtensionAbility") => {
            Some(KitEntry::Default("@ohos:WorkSchedulerExtensionAbility"))
        }
        ("@kit.BackgroundTasksKit", "WorkSchedulerExtensionContext") => Some(KitEntry::Named(
            "@ohos:WorkSchedulerExtensionAbility",
            "WorkSchedulerExtensionContext",
        )),
        ("@kit.BackgroundTasksKit", "backgroundProcessManager") => Some(KitEntry::Default(
            "@ohos:resourceschedule.backgroundProcessManager",
        )),
        ("@kit.BackgroundTasksKit", "backgroundTaskManager") => Some(KitEntry::Default(
            "@ohos:resourceschedule.backgroundTaskManager",
        )),
        ("@kit.BackgroundTasksKit", "bundleState") => Some(KitEntry::Default("@ohos:bundleState")),
        ("@kit.BackgroundTasksKit", "deviceStandby") => {
            Some(KitEntry::Default("@ohos:resourceschedule.deviceStandby"))
        }
        ("@kit.BackgroundTasksKit", "reminderAgent") => {
            Some(KitEntry::Default("@ohos:reminderAgent"))
        }
        ("@kit.BackgroundTasksKit", "reminderAgentManager") => {
            Some(KitEntry::Default("@ohos:reminderAgentManager"))
        }
        ("@kit.BackgroundTasksKit", "usageStatistics") => {
            Some(KitEntry::Default("@ohos:resourceschedule.usageStatistics"))
        }
        ("@kit.BackgroundTasksKit", "workScheduler") => {
            Some(KitEntry::Default("@ohos:resourceschedule.workScheduler"))
        }

        // ── @kit.BasicServicesKit ───────────────────────────────────────────────
        ("@kit.BasicServicesKit", "AsyncCallback") => {
            Some(KitEntry::Named("@ohos:base", "AsyncCallback"))
        }
        ("@kit.BasicServicesKit", "Battery") => Some(KitEntry::Default("@system:battery")),
        ("@kit.BasicServicesKit", "BatteryResponse") => {
            Some(KitEntry::Named("@system:battery", "BatteryResponse"))
        }
        ("@kit.BasicServicesKit", "Brightness") => Some(KitEntry::Default("@system:brightness")),
        ("@kit.BasicServicesKit", "BrightnessModeResponse") => Some(KitEntry::Named(
            "@system:brightness",
            "BrightnessModeResponse",
        )),
        ("@kit.BasicServicesKit", "BrightnessResponse") => {
            Some(KitEntry::Named("@system:brightness", "BrightnessResponse"))
        }
        ("@kit.BasicServicesKit", "BusinessError") => {
            Some(KitEntry::Named("@ohos:base", "BusinessError"))
        }
        ("@kit.BasicServicesKit", "Callback") => Some(KitEntry::Named("@ohos:base", "Callback")),
        ("@kit.BasicServicesKit", "Device") => Some(KitEntry::Default("@system:device")),
        ("@kit.BasicServicesKit", "DeviceResponse") => {
            Some(KitEntry::Named("@system:device", "DeviceResponse"))
        }
        ("@kit.BasicServicesKit", "DownloadRequestOptions") => {
            Some(KitEntry::Named("@system:request", "DownloadRequestOptions"))
        }
        ("@kit.BasicServicesKit", "DownloadResponse") => {
            Some(KitEntry::Named("@system:request", "DownloadResponse"))
        }
        ("@kit.BasicServicesKit", "ErrorCallback") => {
            Some(KitEntry::Named("@ohos:base", "ErrorCallback"))
        }
        ("@kit.BasicServicesKit", "GetBrightnessModeOptions") => Some(KitEntry::Named(
            "@system:brightness",
            "GetBrightnessModeOptions",
        )),
        ("@kit.BasicServicesKit", "GetBrightnessOptions") => Some(KitEntry::Named(
            "@system:brightness",
            "GetBrightnessOptions",
        )),
        ("@kit.BasicServicesKit", "GetDeviceOptions") => {
            Some(KitEntry::Named("@system:device", "GetDeviceOptions"))
        }
        ("@kit.BasicServicesKit", "GetStatusOptions") => {
            Some(KitEntry::Named("@system:battery", "GetStatusOptions"))
        }
        ("@kit.BasicServicesKit", "OnDownloadCompleteOptions") => Some(KitEntry::Named(
            "@system:request",
            "OnDownloadCompleteOptions",
        )),
        ("@kit.BasicServicesKit", "OnDownloadCompleteResponse") => Some(KitEntry::Named(
            "@system:request",
            "OnDownloadCompleteResponse",
        )),
        ("@kit.BasicServicesKit", "PrintExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.ability.PrintExtensionAbility"))
        }
        ("@kit.BasicServicesKit", "Request") => Some(KitEntry::Default("@system:request")),
        ("@kit.BasicServicesKit", "RequestData") => {
            Some(KitEntry::Named("@system:request", "RequestData"))
        }
        ("@kit.BasicServicesKit", "RequestFile") => {
            Some(KitEntry::Named("@system:request", "RequestFile"))
        }
        ("@kit.BasicServicesKit", "SetBrightnessModeOptions") => Some(KitEntry::Named(
            "@system:brightness",
            "SetBrightnessModeOptions",
        )),
        ("@kit.BasicServicesKit", "SetBrightnessOptions") => Some(KitEntry::Named(
            "@system:brightness",
            "SetBrightnessOptions",
        )),
        ("@kit.BasicServicesKit", "SetKeepScreenOnOptions") => Some(KitEntry::Named(
            "@system:brightness",
            "SetKeepScreenOnOptions",
        )),
        ("@kit.BasicServicesKit", "UploadRequestOptions") => {
            Some(KitEntry::Named("@system:request", "UploadRequestOptions"))
        }
        ("@kit.BasicServicesKit", "UploadResponse") => {
            Some(KitEntry::Named("@system:request", "UploadResponse"))
        }
        ("@kit.BasicServicesKit", "appAccount") => {
            Some(KitEntry::Default("@ohos:account.appAccount"))
        }
        ("@kit.BasicServicesKit", "batteryInfo") => Some(KitEntry::Default("@ohos:batteryInfo")),
        ("@kit.BasicServicesKit", "cacheDownload") => {
            Some(KitEntry::Default("@ohos:request.cacheDownload"))
        }
        ("@kit.BasicServicesKit", "commonEventManager") => {
            Some(KitEntry::Default("@ohos:commonEventManager"))
        }
        ("@kit.BasicServicesKit", "customConfig") => {
            Some(KitEntry::Default("@ohos:customization.customConfig"))
        }
        ("@kit.BasicServicesKit", "deviceAttest") => Some(KitEntry::Default("@ohos:deviceAttest")),
        ("@kit.BasicServicesKit", "deviceInfo") => Some(KitEntry::Default("@ohos:deviceInfo")),
        ("@kit.BasicServicesKit", "distributedAccount") => {
            Some(KitEntry::Default("@ohos:account.distributedAccount"))
        }
        ("@kit.BasicServicesKit", "emitter") => Some(KitEntry::Default("@ohos:events.emitter")),
        ("@kit.BasicServicesKit", "osAccount") => {
            Some(KitEntry::Default("@ohos:account.osAccount"))
        }
        ("@kit.BasicServicesKit", "pasteboard") => Some(KitEntry::Default("@ohos:pasteboard")),
        ("@kit.BasicServicesKit", "power") => Some(KitEntry::Default("@ohos:power")),
        ("@kit.BasicServicesKit", "print") => Some(KitEntry::Default("@ohos:print")),
        ("@kit.BasicServicesKit", "request") => Some(KitEntry::Default("@ohos:request")),
        ("@kit.BasicServicesKit", "runningLock") => Some(KitEntry::Default("@ohos:runningLock")),
        ("@kit.BasicServicesKit", "scan") => Some(KitEntry::Default("@ohos:scan")),
        ("@kit.BasicServicesKit", "screenLock") => Some(KitEntry::Default("@ohos:screenLock")),
        ("@kit.BasicServicesKit", "serialManager") => {
            Some(KitEntry::Default("@ohos:usbManager.serial"))
        }
        ("@kit.BasicServicesKit", "settings") => Some(KitEntry::Default("@ohos:settings")),
        ("@kit.BasicServicesKit", "systemDateTime") => {
            Some(KitEntry::Default("@ohos:systemDateTime"))
        }
        ("@kit.BasicServicesKit", "systemLoad") => {
            Some(KitEntry::Default("@ohos:resourceschedule.systemload"))
        }
        ("@kit.BasicServicesKit", "systemTime") => Some(KitEntry::Default("@ohos:systemTime")),
        ("@kit.BasicServicesKit", "thermal") => Some(KitEntry::Default("@ohos:thermal")),
        ("@kit.BasicServicesKit", "usb") => Some(KitEntry::Default("@ohos:usb")),
        ("@kit.BasicServicesKit", "usbManager") => Some(KitEntry::Default("@ohos:usbManager")),
        ("@kit.BasicServicesKit", "wallpaper") => Some(KitEntry::Default("@ohos:wallpaper")),
        ("@kit.BasicServicesKit", "zlib") => Some(KitEntry::Default("@ohos:zlib")),

        // ── @kit.CalendarKit ───────────────────────────────────────────────
        ("@kit.CalendarKit", "calendarManager") => Some(KitEntry::Default("@ohos:calendarManager")),

        // ── @kit.CameraKit ───────────────────────────────────────────────
        ("@kit.CameraKit", "camera") => Some(KitEntry::Default("@ohos:multimedia.camera")),
        ("@kit.CameraKit", "cameraPicker") => {
            Some(KitEntry::Default("@ohos:multimedia.cameraPicker"))
        }

        // ── @kit.ConnectivityKit ───────────────────────────────────────────────
        ("@kit.ConnectivityKit", "BLEFoundResponse") => {
            Some(KitEntry::Named("@system:bluetooth", "BLEFoundResponse"))
        }
        ("@kit.ConnectivityKit", "Bluetooth") => Some(KitEntry::Default("@system:bluetooth")),
        ("@kit.ConnectivityKit", "BluetoothDevice") => {
            Some(KitEntry::Named("@system:bluetooth", "BluetoothDevice"))
        }
        ("@kit.ConnectivityKit", "StartBLEScanOptions") => {
            Some(KitEntry::Named("@system:bluetooth", "StartBLEScanOptions"))
        }
        ("@kit.ConnectivityKit", "StopBLEScanOptions") => {
            Some(KitEntry::Named("@system:bluetooth", "StopBLEScanOptions"))
        }
        ("@kit.ConnectivityKit", "SubscribeBLEFoundOptions") => Some(KitEntry::Named(
            "@system:bluetooth",
            "SubscribeBLEFoundOptions",
        )),
        ("@kit.ConnectivityKit", "a2dp") => Some(KitEntry::Default("@ohos:bluetooth.a2dp")),
        ("@kit.ConnectivityKit", "access") => Some(KitEntry::Default("@ohos:bluetooth.access")),
        ("@kit.ConnectivityKit", "baseProfile") => {
            Some(KitEntry::Default("@ohos:bluetooth.baseProfile"))
        }
        ("@kit.ConnectivityKit", "ble") => Some(KitEntry::Default("@ohos:bluetooth.ble")),
        ("@kit.ConnectivityKit", "bluetooth") => Some(KitEntry::Default("@ohos:bluetooth")),
        ("@kit.ConnectivityKit", "bluetoothManager") => {
            Some(KitEntry::Default("@ohos:bluetoothManager"))
        }
        ("@kit.ConnectivityKit", "cardEmulation") => {
            Some(KitEntry::Default("@ohos:nfc.cardEmulation"))
        }
        ("@kit.ConnectivityKit", "common") => Some(KitEntry::Default("@ohos:bluetooth.common")),
        ("@kit.ConnectivityKit", "connectedTag") => Some(KitEntry::Default("@ohos:connectedTag")),
        ("@kit.ConnectivityKit", "connection") => {
            Some(KitEntry::Default("@ohos:bluetooth.connection"))
        }
        ("@kit.ConnectivityKit", "constant") => Some(KitEntry::Default("@ohos:bluetooth.constant")),
        ("@kit.ConnectivityKit", "hfp") => Some(KitEntry::Default("@ohos:bluetooth.hfp")),
        ("@kit.ConnectivityKit", "hid") => Some(KitEntry::Default("@ohos:bluetooth.hid")),
        ("@kit.ConnectivityKit", "map") => Some(KitEntry::Default("@ohos:bluetooth.map")),
        ("@kit.ConnectivityKit", "nfcController") => {
            Some(KitEntry::Default("@ohos:nfc.controller"))
        }
        ("@kit.ConnectivityKit", "omapi") => Some(KitEntry::Default("@ohos:secureElement")),
        ("@kit.ConnectivityKit", "opp") => Some(KitEntry::Default("@ohos:bluetooth.opp")),
        ("@kit.ConnectivityKit", "pan") => Some(KitEntry::Default("@ohos:bluetooth.pan")),
        ("@kit.ConnectivityKit", "pbap") => Some(KitEntry::Default("@ohos:bluetooth.pbap")),
        ("@kit.ConnectivityKit", "socket") => Some(KitEntry::Default("@ohos:bluetooth.socket")),
        ("@kit.ConnectivityKit", "tag") => Some(KitEntry::Default("@ohos:nfc.tag")),
        ("@kit.ConnectivityKit", "wearDetection") => {
            Some(KitEntry::Default("@ohos:bluetooth.wearDetection"))
        }
        ("@kit.ConnectivityKit", "wifi") => Some(KitEntry::Default("@ohos:wifi")),
        ("@kit.ConnectivityKit", "wifiManager") => Some(KitEntry::Default("@ohos:wifiManager")),
        ("@kit.ConnectivityKit", "wifiManagerExt") => {
            Some(KitEntry::Default("@ohos:wifiManagerExt"))
        }
        ("@kit.ConnectivityKit", "wifiext") => Some(KitEntry::Default("@ohos:wifiext")),

        // ── @kit.ContactsKit ───────────────────────────────────────────────
        ("@kit.ContactsKit", "contact") => Some(KitEntry::Default("@ohos:contact")),

        // ── @kit.CoreFileKit ───────────────────────────────────────────────
        ("@kit.CoreFileKit", "BackupExtensionAbility") => Some(KitEntry::Default(
            "@ohos:application.BackupExtensionAbility",
        )),
        ("@kit.CoreFileKit", "BackupExtensionContext") => {
            Some(KitEntry::Default("@ohos:file.BackupExtensionContext"))
        }
        ("@kit.CoreFileKit", "BundleVersion") => Some(KitEntry::Named(
            "@ohos:application.BackupExtensionAbility",
            "BundleVersion",
        )),
        ("@kit.CoreFileKit", "ConflictFiles") => {
            Some(KitEntry::Named("@ohos:file.fs", "ConflictFiles"))
        }
        ("@kit.CoreFileKit", "DfsListeners") => {
            Some(KitEntry::Named("@ohos:file.fs", "DfsListeners"))
        }
        ("@kit.CoreFileKit", "Environment") => Some(KitEntry::Default("@ohos:file.environment")),
        ("@kit.CoreFileKit", "Filter") => Some(KitEntry::Named("@ohos:file.fs", "Filter")),
        ("@kit.CoreFileKit", "ListFileOptions") => {
            Some(KitEntry::Named("@ohos:file.fs", "ListFileOptions"))
        }
        ("@kit.CoreFileKit", "Options") => Some(KitEntry::Named("@ohos:file.fs", "Options")),
        ("@kit.CoreFileKit", "ReadOptions") => {
            Some(KitEntry::Named("@ohos:file.fs", "ReadOptions"))
        }
        ("@kit.CoreFileKit", "ReadTextOptions") => {
            Some(KitEntry::Named("@ohos:file.fs", "ReadTextOptions"))
        }
        ("@kit.CoreFileKit", "ReaderIteratorResult") => {
            Some(KitEntry::Named("@ohos:file.fs", "ReaderIteratorResult"))
        }
        ("@kit.CoreFileKit", "TaskSignal") => Some(KitEntry::Named("@ohos:file.fs", "TaskSignal")),
        ("@kit.CoreFileKit", "WatchEvent") => Some(KitEntry::Named("@ohos:file.fs", "WatchEvent")),
        ("@kit.CoreFileKit", "WatchEventListener") => {
            Some(KitEntry::Named("@ohos:file.fs", "WatchEventListener"))
        }
        ("@kit.CoreFileKit", "Watcher") => Some(KitEntry::Named("@ohos:file.fs", "Watcher")),
        ("@kit.CoreFileKit", "WriteOptions") => {
            Some(KitEntry::Named("@ohos:file.fs", "WriteOptions"))
        }
        ("@kit.CoreFileKit", "cloudSync") => Some(KitEntry::Default("@ohos:file.cloudSync")),
        ("@kit.CoreFileKit", "cloudSyncManager") => {
            Some(KitEntry::Default("@ohos:file.cloudSyncManager"))
        }
        ("@kit.CoreFileKit", "fileAccess") => Some(KitEntry::Default("@ohos:file.fileAccess")),
        ("@kit.CoreFileKit", "fileIo") => Some(KitEntry::Default("@ohos:file.fs")),
        ("@kit.CoreFileKit", "fileShare") => Some(KitEntry::Default("@ohos:fileshare")),
        ("@kit.CoreFileKit", "fileUri") => Some(KitEntry::Default("@ohos:file.fileuri")),
        ("@kit.CoreFileKit", "hash") => Some(KitEntry::Default("@ohos:file.hash")),
        ("@kit.CoreFileKit", "keyManager") => Some(KitEntry::Default("@ohos:file.keyManager")),
        ("@kit.CoreFileKit", "picker") => Some(KitEntry::Default("@ohos:file.picker")),
        ("@kit.CoreFileKit", "securityLabel") => {
            Some(KitEntry::Default("@ohos:file.securityLabel"))
        }
        ("@kit.CoreFileKit", "statfs") => Some(KitEntry::Default("@ohos:file.statvfs")),
        ("@kit.CoreFileKit", "storageStatistics") => {
            Some(KitEntry::Default("@ohos:file.storageStatistics"))
        }

        // ── @kit.CryptoArchitectureKit ───────────────────────────────────────────────
        ("@kit.CryptoArchitectureKit", "Cipher") => Some(KitEntry::Default("@system:cipher")),
        ("@kit.CryptoArchitectureKit", "CipherAesOptions") => {
            Some(KitEntry::Named("@system:cipher", "CipherAesOptions"))
        }
        ("@kit.CryptoArchitectureKit", "CipherResponse") => {
            Some(KitEntry::Named("@system:cipher", "CipherResponse"))
        }
        ("@kit.CryptoArchitectureKit", "CipherRsaOptions") => {
            Some(KitEntry::Named("@system:cipher", "CipherRsaOptions"))
        }
        ("@kit.CryptoArchitectureKit", "cryptoFramework") => {
            Some(KitEntry::Default("@ohos:security.cryptoFramework"))
        }

        // ── @kit.DataLossPreventionKit ───────────────────────────────────────────────
        ("@kit.DataLossPreventionKit", "dlpPermission") => {
            Some(KitEntry::Default("@ohos:dlpPermission"))
        }

        // ── @kit.DataProtectionKit ───────────────────────────────────────────────
        ("@kit.DataProtectionKit", "dlpPermission") => {
            Some(KitEntry::Default("@ohos:dlpPermission"))
        }
        ("@kit.DataProtectionKit", "identifySensitiveContent") => {
            Some(KitEntry::Default("@ohos:security.identifySensitiveContent"))
        }

        // ── @kit.DeviceCertificateKit ───────────────────────────────────────────────
        ("@kit.DeviceCertificateKit", "cert") => Some(KitEntry::Default("@ohos:security.cert")),
        ("@kit.DeviceCertificateKit", "certificateManager") => {
            Some(KitEntry::Default("@ohos:security.certManager"))
        }
        ("@kit.DeviceCertificateKit", "certificateManagerDialog") => {
            Some(KitEntry::Default("@ohos:security.certManagerDialog"))
        }

        // ── @kit.DistributedServiceKit ───────────────────────────────────────────────
        ("@kit.DistributedServiceKit", "DistributedExtensionAbility") => Some(KitEntry::Default(
            "@ohos:application.DistributedExtensionAbility",
        )),
        ("@kit.DistributedServiceKit", "DistributedExtensionContext") => Some(KitEntry::Default(
            "@ohos:application.DistributedExtensionContext",
        )),
        ("@kit.DistributedServiceKit", "abilityConnectionManager") => Some(KitEntry::Default(
            "@ohos:distributedsched.abilityConnectionManager",
        )),
        ("@kit.DistributedServiceKit", "deviceManager") => {
            Some(KitEntry::Default("@ohos:distributedHardware.deviceManager"))
        }
        ("@kit.DistributedServiceKit", "distributedDeviceManager") => {
            Some(KitEntry::Default("@ohos:distributedDeviceManager"))
        }
        ("@kit.DistributedServiceKit", "linkEnhance") => {
            Some(KitEntry::Default("@ohos:distributedsched.linkEnhance"))
        }
        ("@kit.DistributedServiceKit", "proxyChannelManager") => Some(KitEntry::Default(
            "@ohos:distributedsched.proxyChannelManager",
        )),

        // ── @kit.DriverDevelopmentKit ───────────────────────────────────────────────
        ("@kit.DriverDevelopmentKit", "DriverExtensionAbility") => Some(KitEntry::Default(
            "@ohos:app.ability.DriverExtensionAbility",
        )),
        ("@kit.DriverDevelopmentKit", "DriverExtensionContext") => Some(KitEntry::Named(
            "@ohos:app.ability.DriverExtensionAbility",
            "DriverExtensionContext",
        )),
        ("@kit.DriverDevelopmentKit", "deviceManager") => {
            Some(KitEntry::Default("@ohos:driver.deviceManager"))
        }

        // ── @kit.DrmKit ───────────────────────────────────────────────
        ("@kit.DrmKit", "drm") => Some(KitEntry::Default("@ohos:multimedia.drm")),

        // ── @kit.FormKit ───────────────────────────────────────────────
        ("@kit.FormKit", "FormEditExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.form.FormEditExtensionAbility"))
        }
        ("@kit.FormKit", "FormExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.form.FormExtensionAbility"))
        }
        ("@kit.FormKit", "LiveFormExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.form.LiveFormExtensionAbility"))
        }
        ("@kit.FormKit", "LiveFormInfo") => Some(KitEntry::Named(
            "@ohos:app.form.LiveFormExtensionAbility",
            "LiveFormInfo",
        )),
        ("@kit.FormKit", "formBindingData") => {
            Some(KitEntry::Default("@ohos:app.form.formBindingData"))
        }
        ("@kit.FormKit", "formError") => Some(KitEntry::Default("@ohos:application.formError")),
        ("@kit.FormKit", "formInfo") => Some(KitEntry::Default("@ohos:app.form.formInfo")),
        ("@kit.FormKit", "formProvider") => Some(KitEntry::Default("@ohos:app.form.formProvider")),

        // ── @kit.IMEKit ───────────────────────────────────────────────
        ("@kit.IMEKit", "InputMethodExtensionAbility") => {
            Some(KitEntry::Default("@ohos:InputMethodExtensionAbility"))
        }
        ("@kit.IMEKit", "InputMethodExtensionContext") => {
            Some(KitEntry::Default("@ohos:InputMethodExtensionContext"))
        }
        ("@kit.IMEKit", "InputMethodListDialog") => Some(KitEntry::Named(
            "@ohos:inputMethodList",
            "InputMethodListDialog",
        )),
        ("@kit.IMEKit", "InputMethodSubtype") => {
            Some(KitEntry::Default("@ohos:InputMethodSubtype"))
        }
        ("@kit.IMEKit", "PanelFlag") => {
            Some(KitEntry::Named("@ohos:inputMethod.Panel", "PanelFlag"))
        }
        ("@kit.IMEKit", "PanelInfo") => {
            Some(KitEntry::Named("@ohos:inputMethod.Panel", "PanelInfo"))
        }
        ("@kit.IMEKit", "PanelType") => {
            Some(KitEntry::Named("@ohos:inputMethod.Panel", "PanelType"))
        }
        ("@kit.IMEKit", "Pattern") => Some(KitEntry::Named("@ohos:inputMethodList", "Pattern")),
        ("@kit.IMEKit", "PatternOptions") => {
            Some(KitEntry::Named("@ohos:inputMethodList", "PatternOptions"))
        }
        ("@kit.IMEKit", "inputMethod") => Some(KitEntry::Default("@ohos:inputMethod")),
        ("@kit.IMEKit", "inputMethodEngine") => Some(KitEntry::Default("@ohos:inputMethodEngine")),

        // ── @kit.IPCKit ───────────────────────────────────────────────
        ("@kit.IPCKit", "rpc") => Some(KitEntry::Default("@ohos:rpc")),

        // ── @kit.ImageKit ───────────────────────────────────────────────
        ("@kit.ImageKit", "image") => Some(KitEntry::Default("@ohos:multimedia.image")),
        ("@kit.ImageKit", "sendableImage") => {
            Some(KitEntry::Default("@ohos:multimedia.sendableImage"))
        }
        ("@kit.ImageKit", "videoProcessingEngine") => {
            Some(KitEntry::Default("@ohos:multimedia.videoProcessingEngine"))
        }

        // ── @kit.InputKit ───────────────────────────────────────────────
        ("@kit.InputKit", "Action") => {
            Some(KitEntry::Named("@ohos:multimodalInput.keyEvent", "Action"))
        }
        ("@kit.InputKit", "ActionType") => Some(KitEntry::Named(
            "@ohos:multimodalInput.gestureEvent",
            "ActionType",
        )),
        ("@kit.InputKit", "Axis") => {
            Some(KitEntry::Named("@ohos:multimodalInput.mouseEvent", "Axis"))
        }
        ("@kit.InputKit", "AxisValue") => Some(KitEntry::Named(
            "@ohos:multimodalInput.mouseEvent",
            "AxisValue",
        )),
        ("@kit.InputKit", "Button") => Some(KitEntry::Named(
            "@ohos:multimodalInput.mouseEvent",
            "Button",
        )),
        ("@kit.InputKit", "FourFingersSwipe") => Some(KitEntry::Named(
            "@ohos:multimodalInput.gestureEvent",
            "FourFingersSwipe",
        )),
        ("@kit.InputKit", "InputEvent") => Some(KitEntry::Named(
            "@ohos:multimodalInput.inputEvent",
            "InputEvent",
        )),
        ("@kit.InputKit", "IntentionCode") => Some(KitEntry::Named(
            "@ohos:multimodalInput.intentionCode",
            "IntentionCode",
        )),
        ("@kit.InputKit", "Key") => Some(KitEntry::Named("@ohos:multimodalInput.keyEvent", "Key")),
        ("@kit.InputKit", "KeyAction") => Some(KitEntry::Named(
            "@ohos:multimodalInput.touchEvent",
            "Action",
        )),
        ("@kit.InputKit", "KeyCode") => {
            Some(KitEntry::Named("@ohos:multimodalInput.keyCode", "KeyCode"))
        }
        ("@kit.InputKit", "KeyEvent") => Some(KitEntry::Named(
            "@ohos:multimodalInput.keyEvent",
            "KeyEvent",
        )),
        ("@kit.InputKit", "MouseAction") => Some(KitEntry::Named(
            "@ohos:multimodalInput.mouseEvent",
            "Action",
        )),
        ("@kit.InputKit", "MouseEvent") => Some(KitEntry::Named(
            "@ohos:multimodalInput.mouseEvent",
            "MouseEvent",
        )),
        ("@kit.InputKit", "MouseToolType") => Some(KitEntry::Named(
            "@ohos:multimodalInput.mouseEvent",
            "ToolType",
        )),
        ("@kit.InputKit", "Pinch") => Some(KitEntry::Named(
            "@ohos:multimodalInput.gestureEvent",
            "Pinch",
        )),
        ("@kit.InputKit", "Rotate") => Some(KitEntry::Named(
            "@ohos:multimodalInput.gestureEvent",
            "Rotate",
        )),
        ("@kit.InputKit", "SourceType") => Some(KitEntry::Named(
            "@ohos:multimodalInput.touchEvent",
            "SourceType",
        )),
        ("@kit.InputKit", "ThreeFingersSwipe") => Some(KitEntry::Named(
            "@ohos:multimodalInput.gestureEvent",
            "ThreeFingersSwipe",
        )),
        ("@kit.InputKit", "ThreeFingersTap") => Some(KitEntry::Named(
            "@ohos:multimodalInput.gestureEvent",
            "ThreeFingersTap",
        )),
        ("@kit.InputKit", "ToolType") => Some(KitEntry::Named(
            "@ohos:multimodalInput.touchEvent",
            "ToolType",
        )),
        ("@kit.InputKit", "Touch") => {
            Some(KitEntry::Named("@ohos:multimodalInput.touchEvent", "Touch"))
        }
        ("@kit.InputKit", "TouchEvent") => Some(KitEntry::Named(
            "@ohos:multimodalInput.touchEvent",
            "TouchEvent",
        )),
        ("@kit.InputKit", "infraredEmitter") => {
            Some(KitEntry::Default("@ohos:multimodalInput.infraredEmitter"))
        }
        ("@kit.InputKit", "inputConsumer") => {
            Some(KitEntry::Default("@ohos:multimodalInput.inputConsumer"))
        }
        ("@kit.InputKit", "inputDevice") => {
            Some(KitEntry::Default("@ohos:multimodalInput.inputDevice"))
        }
        ("@kit.InputKit", "inputDeviceCooperate") => Some(KitEntry::Default(
            "@ohos:multimodalInput.inputDeviceCooperate",
        )),
        ("@kit.InputKit", "pointer") => Some(KitEntry::Default("@ohos:multimodalInput.pointer")),

        // ── @kit.LocalizationKit ───────────────────────────────────────────────
        ("@kit.LocalizationKit", "i18n") => Some(KitEntry::Default("@ohos:i18n")),
        ("@kit.LocalizationKit", "intl") => Some(KitEntry::Default("@ohos:intl")),
        ("@kit.LocalizationKit", "resourceManager") => {
            Some(KitEntry::Default("@ohos:resourceManager"))
        }
        ("@kit.LocalizationKit", "sendableResourceManager") => {
            Some(KitEntry::Default("@ohos:sendableResourceManager"))
        }

        // ── @kit.LocationKit ───────────────────────────────────────────────
        ("@kit.LocationKit", "FenceExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.ability.FenceExtensionAbility"))
        }
        ("@kit.LocationKit", "FenceExtensionContext") => {
            Some(KitEntry::Default("@ohos:app.ability.FenceExtensionContext"))
        }
        ("@kit.LocationKit", "Geolocation") => Some(KitEntry::Default("@system:geolocation")),
        ("@kit.LocationKit", "GeolocationResponse") => Some(KitEntry::Named(
            "@system:geolocation",
            "GeolocationResponse",
        )),
        ("@kit.LocationKit", "GetLocationOption") => {
            Some(KitEntry::Named("@system:geolocation", "GetLocationOption"))
        }
        ("@kit.LocationKit", "GetLocationTypeOption") => Some(KitEntry::Named(
            "@system:geolocation",
            "GetLocationTypeOption",
        )),
        ("@kit.LocationKit", "GetLocationTypeResponse") => Some(KitEntry::Named(
            "@system:geolocation",
            "GetLocationTypeResponse",
        )),
        ("@kit.LocationKit", "SubscribeLocationOption") => Some(KitEntry::Named(
            "@system:geolocation",
            "SubscribeLocationOption",
        )),
        ("@kit.LocationKit", "geoLocationManager") => {
            Some(KitEntry::Default("@ohos:geoLocationManager"))
        }
        ("@kit.LocationKit", "geolocation") => Some(KitEntry::Default("@ohos:geolocation")),

        // ── @kit.MDMKit ───────────────────────────────────────────────
        ("@kit.MDMKit", "EnterpriseAdminExtensionAbility") => Some(KitEntry::Default(
            "@ohos:enterprise.EnterpriseAdminExtensionAbility",
        )),
        ("@kit.MDMKit", "accountManager") => {
            Some(KitEntry::Default("@ohos:enterprise.accountManager"))
        }
        ("@kit.MDMKit", "adminManager") => Some(KitEntry::Default("@ohos:enterprise.adminManager")),
        ("@kit.MDMKit", "applicationManager") => {
            Some(KitEntry::Default("@ohos:enterprise.applicationManager"))
        }
        ("@kit.MDMKit", "bluetoothManager") => {
            Some(KitEntry::Default("@ohos:enterprise.bluetoothManager"))
        }
        ("@kit.MDMKit", "browser") => Some(KitEntry::Default("@ohos:enterprise.browser")),
        ("@kit.MDMKit", "bundleManager") => {
            Some(KitEntry::Default("@ohos:enterprise.bundleManager"))
        }
        ("@kit.MDMKit", "deviceControl") => {
            Some(KitEntry::Default("@ohos:enterprise.deviceControl"))
        }
        ("@kit.MDMKit", "deviceInfo") => Some(KitEntry::Default("@ohos:enterprise.deviceInfo")),
        ("@kit.MDMKit", "deviceSettings") => {
            Some(KitEntry::Default("@ohos:enterprise.deviceSettings"))
        }
        ("@kit.MDMKit", "locationManager") => {
            Some(KitEntry::Default("@ohos:enterprise.locationManager"))
        }
        ("@kit.MDMKit", "networkManager") => {
            Some(KitEntry::Default("@ohos:enterprise.networkManager"))
        }
        ("@kit.MDMKit", "restrictions") => Some(KitEntry::Default("@ohos:enterprise.restrictions")),
        ("@kit.MDMKit", "securityManager") => {
            Some(KitEntry::Default("@ohos:enterprise.securityManager"))
        }
        ("@kit.MDMKit", "systemManager") => {
            Some(KitEntry::Default("@ohos:enterprise.systemManager"))
        }
        ("@kit.MDMKit", "telephonyManager") => {
            Some(KitEntry::Default("@ohos:enterprise.telephonyManager"))
        }
        ("@kit.MDMKit", "usbManager") => Some(KitEntry::Default("@ohos:enterprise.usbManager")),
        ("@kit.MDMKit", "wifiManager") => Some(KitEntry::Default("@ohos:enterprise.wifiManager")),

        // ── @kit.MechanicKit ───────────────────────────────────────────────
        ("@kit.MechanicKit", "mechanicManager") => Some(KitEntry::Default(
            "@ohos:distributedHardware.mechanicManager",
        )),

        // ── @kit.MediaKit ───────────────────────────────────────────────
        ("@kit.MediaKit", "media") => Some(KitEntry::Default("@ohos:multimedia.media")),

        // ── @kit.MediaLibraryKit ───────────────────────────────────────────────
        ("@kit.MediaLibraryKit", "AlbumInfo") => Some(KitEntry::Named(
            "@ohos:file.AlbumPickerComponent",
            "AlbumInfo",
        )),
        ("@kit.MediaLibraryKit", "AlbumPickerComponent") => Some(KitEntry::Named(
            "@ohos:file.AlbumPickerComponent",
            "AlbumPickerComponent",
        )),
        ("@kit.MediaLibraryKit", "AlbumPickerController") => Some(KitEntry::Named(
            "@ohos:file.AlbumPickerComponent",
            "AlbumPickerController",
        )),
        ("@kit.MediaLibraryKit", "AlbumPickerOptions") => Some(KitEntry::Named(
            "@ohos:file.AlbumPickerComponent",
            "AlbumPickerOptions",
        )),
        ("@kit.MediaLibraryKit", "AnimatorParams") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "AnimatorParams",
        )),
        ("@kit.MediaLibraryKit", "BaseItemInfo") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "BaseItemInfo",
        )),
        ("@kit.MediaLibraryKit", "ClickType") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "ClickType",
        )),
        ("@kit.MediaLibraryKit", "CurrentAlbumDeletedCallback") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "CurrentAlbumDeletedCallback",
        )),
        ("@kit.MediaLibraryKit", "DataType") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "DataType",
        )),
        ("@kit.MediaLibraryKit", "EmptyAreaClickCallback") => Some(KitEntry::Named(
            "@ohos:file.AlbumPickerComponent",
            "EmptyAreaClickCallback",
        )),
        ("@kit.MediaLibraryKit", "ExceedMaxSelectedCallback") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "ExceedMaxSelectedCallback",
        )),
        ("@kit.MediaLibraryKit", "ItemInfo") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "ItemInfo",
        )),
        ("@kit.MediaLibraryKit", "ItemType") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "ItemType",
        )),
        ("@kit.MediaLibraryKit", "ItemsDeletedCallback") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "ItemsDeletedCallback",
        )),
        ("@kit.MediaLibraryKit", "MaxCountType") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "MaxCountType",
        )),
        ("@kit.MediaLibraryKit", "MaxSelected") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "MaxSelected",
        )),
        ("@kit.MediaLibraryKit", "MovingPhotoView") => Some(KitEntry::Named(
            "@ohos:multimedia.movingphotoview",
            "MovingPhotoView",
        )),
        ("@kit.MediaLibraryKit", "MovingPhotoViewAttribute") => Some(KitEntry::Named(
            "@ohos:multimedia.movingphotoview",
            "MovingPhotoViewAttribute",
        )),
        ("@kit.MediaLibraryKit", "MovingPhotoViewController") => Some(KitEntry::Named(
            "@ohos:multimedia.movingphotoview",
            "MovingPhotoViewController",
        )),
        ("@kit.MediaLibraryKit", "PhotoBrowserInfo") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PhotoBrowserInfo",
        )),
        ("@kit.MediaLibraryKit", "PhotoBrowserRange") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PhotoBrowserRange",
        )),
        ("@kit.MediaLibraryKit", "PhotoBrowserUIElement") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PhotoBrowserUIElement",
        )),
        ("@kit.MediaLibraryKit", "PhotoPickerComponent") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PhotoPickerComponent",
        )),
        ("@kit.MediaLibraryKit", "PhotoSource") => Some(KitEntry::Named(
            "@ohos:file.RecentPhotoComponent",
            "PhotoSource",
        )),
        ("@kit.MediaLibraryKit", "PickerColorMode") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PickerColorMode",
        )),
        ("@kit.MediaLibraryKit", "PickerController") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PickerController",
        )),
        ("@kit.MediaLibraryKit", "PickerOptions") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PickerOptions",
        )),
        ("@kit.MediaLibraryKit", "PickerOrientation") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "PickerOrientation",
        )),
        ("@kit.MediaLibraryKit", "RecentPhotoCheckResultCallback") => Some(KitEntry::Named(
            "@ohos:file.RecentPhotoComponent",
            "RecentPhotoCheckResultCallback",
        )),
        ("@kit.MediaLibraryKit", "RecentPhotoClickCallback") => Some(KitEntry::Named(
            "@ohos:file.RecentPhotoComponent",
            "RecentPhotoClickCallback",
        )),
        ("@kit.MediaLibraryKit", "RecentPhotoComponent") => Some(KitEntry::Named(
            "@ohos:file.RecentPhotoComponent",
            "RecentPhotoComponent",
        )),
        ("@kit.MediaLibraryKit", "RecentPhotoOptions") => Some(KitEntry::Named(
            "@ohos:file.RecentPhotoComponent",
            "RecentPhotoOptions",
        )),
        ("@kit.MediaLibraryKit", "ReminderMode") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "ReminderMode",
        )),
        ("@kit.MediaLibraryKit", "SelectMode") => Some(KitEntry::Named(
            "@ohos:file.PhotoPickerComponent",
            "SelectMode",
        )),
        ("@kit.MediaLibraryKit", "photoAccessHelper") => {
            Some(KitEntry::Default("@ohos:file.photoAccessHelper"))
        }
        ("@kit.MediaLibraryKit", "sendablePhotoAccessHelper") => {
            Some(KitEntry::Default("@ohos:file.sendablePhotoAccessHelper"))
        }

        // ── @kit.MindSporeLiteKit ───────────────────────────────────────────────
        ("@kit.MindSporeLiteKit", "mindSporeLite") => {
            Some(KitEntry::Default("@ohos:ai.mindSporeLite"))
        }

        // ── @kit.MultimodalAwarenessKit ───────────────────────────────────────────────
        ("@kit.MultimodalAwarenessKit", "deviceStatus") => {
            Some(KitEntry::Default("@ohos:multimodalAwareness.deviceStatus"))
        }
        ("@kit.MultimodalAwarenessKit", "metadataBinding") => Some(KitEntry::Default(
            "@ohos:multimodalAwareness.metadataBinding",
        )),
        ("@kit.MultimodalAwarenessKit", "motion") => {
            Some(KitEntry::Default("@ohos:multimodalAwareness.motion"))
        }
        ("@kit.MultimodalAwarenessKit", "onScreen") => {
            Some(KitEntry::Default("@ohos:multimodalAwareness.onScreen"))
        }
        ("@kit.MultimodalAwarenessKit", "stationary") => {
            Some(KitEntry::Default("@ohos:stationary"))
        }
        ("@kit.MultimodalAwarenessKit", "userStatus") => {
            Some(KitEntry::Default("@ohos:multimodalAwareness.userStatus"))
        }

        // ── @kit.NetworkKit ───────────────────────────────────────────────
        ("@kit.NetworkKit", "VpnExtensionAbility") => {
            Some(KitEntry::Default("@ohos:app.ability.VpnExtensionAbility"))
        }
        ("@kit.NetworkKit", "VpnExtensionContext") => Some(KitEntry::Named(
            "@ohos:app.ability.VpnExtensionAbility",
            "VpnExtensionContext",
        )),
        ("@kit.NetworkKit", "connection") => Some(KitEntry::Default("@ohos:net.connection")),
        ("@kit.NetworkKit", "eap") => Some(KitEntry::Default("@ohos:net.eap")),
        ("@kit.NetworkKit", "ethernet") => Some(KitEntry::Default("@ohos:net.ethernet")),
        ("@kit.NetworkKit", "http") => Some(KitEntry::Default("@ohos:net.http")),
        ("@kit.NetworkKit", "mdns") => Some(KitEntry::Default("@ohos:net.mdns")),
        ("@kit.NetworkKit", "netFirewall") => Some(KitEntry::Default("@ohos:net.netFirewall")),
        ("@kit.NetworkKit", "networkSecurity") => {
            Some(KitEntry::Default("@ohos:net.networkSecurity"))
        }
        ("@kit.NetworkKit", "policy") => Some(KitEntry::Default("@ohos:net.policy")),
        ("@kit.NetworkKit", "sharing") => Some(KitEntry::Default("@ohos:net.sharing")),
        ("@kit.NetworkKit", "socket") => Some(KitEntry::Default("@ohos:net.socket")),
        ("@kit.NetworkKit", "statistics") => Some(KitEntry::Default("@ohos:net.statistics")),
        ("@kit.NetworkKit", "vpn") => Some(KitEntry::Default("@ohos:net.vpn")),
        ("@kit.NetworkKit", "vpnExtension") => Some(KitEntry::Default("@ohos:net.vpnExtension")),
        ("@kit.NetworkKit", "webSocket") => Some(KitEntry::Default("@ohos:net.webSocket")),

        // ── @kit.NotificationKit ───────────────────────────────────────────────
        ("@kit.NotificationKit", "ActionResult") => {
            Some(KitEntry::Named("@system:notification", "ActionResult"))
        }
        ("@kit.NotificationKit", "Notification") => Some(KitEntry::Default("@system:notification")),
        ("@kit.NotificationKit", "ShowNotificationOptions") => Some(KitEntry::Named(
            "@system:notification",
            "ShowNotificationOptions",
        )),
        ("@kit.NotificationKit", "notificationManager") => {
            Some(KitEntry::Default("@ohos:notificationManager"))
        }

        // ── @kit.PerformanceAnalysisKit ───────────────────────────────────────────────
        ("@kit.PerformanceAnalysisKit", "FaultLogExtensionAbility") => Some(KitEntry::Default(
            "@ohos:hiviewdfx.FaultLogExtensionAbility",
        )),
        ("@kit.PerformanceAnalysisKit", "FaultLogExtensionContext") => Some(KitEntry::Default(
            "@ohos:hiviewdfx.FaultLogExtensionContext",
        )),
        ("@kit.PerformanceAnalysisKit", "FaultLogger") => {
            Some(KitEntry::Default("@ohos:faultLogger"))
        }
        ("@kit.PerformanceAnalysisKit", "hiAppEvent") => {
            Some(KitEntry::Default("@ohos:hiviewdfx.hiAppEvent"))
        }
        ("@kit.PerformanceAnalysisKit", "hiTraceChain") => {
            Some(KitEntry::Default("@ohos:hiTraceChain"))
        }
        ("@kit.PerformanceAnalysisKit", "hiTraceMeter") => {
            Some(KitEntry::Default("@ohos:hiTraceMeter"))
        }
        ("@kit.PerformanceAnalysisKit", "hichecker") => Some(KitEntry::Default("@ohos:hichecker")),
        ("@kit.PerformanceAnalysisKit", "hidebug") => Some(KitEntry::Default("@ohos:hidebug")),
        ("@kit.PerformanceAnalysisKit", "hilog") => Some(KitEntry::Default("@ohos:hilog")),
        ("@kit.PerformanceAnalysisKit", "jsLeakWatcher") => {
            Some(KitEntry::Default("@ohos:hiviewdfx.jsLeakWatcher"))
        }

        // ── @kit.SensorServiceKit ───────────────────────────────────────────────
        ("@kit.SensorServiceKit", "AccelerometerResponse") => {
            Some(KitEntry::Named("@system:sensor", "AccelerometerResponse"))
        }
        ("@kit.SensorServiceKit", "BarometerResponse") => {
            Some(KitEntry::Named("@system:sensor", "BarometerResponse"))
        }
        ("@kit.SensorServiceKit", "CompassResponse") => {
            Some(KitEntry::Named("@system:sensor", "CompassResponse"))
        }
        ("@kit.SensorServiceKit", "DeviceOrientationResponse") => Some(KitEntry::Named(
            "@system:sensor",
            "DeviceOrientationResponse",
        )),
        ("@kit.SensorServiceKit", "GetOnBodyStateOptions") => {
            Some(KitEntry::Named("@system:sensor", "GetOnBodyStateOptions"))
        }
        ("@kit.SensorServiceKit", "GyroscopeResponse") => {
            Some(KitEntry::Named("@system:sensor", "GyroscopeResponse"))
        }
        ("@kit.SensorServiceKit", "HeartRateResponse") => {
            Some(KitEntry::Named("@system:sensor", "HeartRateResponse"))
        }
        ("@kit.SensorServiceKit", "LightResponse") => {
            Some(KitEntry::Named("@system:sensor", "LightResponse"))
        }
        ("@kit.SensorServiceKit", "OnBodyStateResponse") => {
            Some(KitEntry::Named("@system:sensor", "OnBodyStateResponse"))
        }
        ("@kit.SensorServiceKit", "ProximityResponse") => {
            Some(KitEntry::Named("@system:sensor", "ProximityResponse"))
        }
        ("@kit.SensorServiceKit", "Sensor") => Some(KitEntry::Default("@system:sensor")),
        ("@kit.SensorServiceKit", "StepCounterResponse") => {
            Some(KitEntry::Named("@system:sensor", "StepCounterResponse"))
        }
        ("@kit.SensorServiceKit", "SubscribeBarometerOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "SubscribeBarometerOptions",
        )),
        ("@kit.SensorServiceKit", "SubscribeCompassOptions") => {
            Some(KitEntry::Named("@system:sensor", "SubscribeCompassOptions"))
        }
        ("@kit.SensorServiceKit", "SubscribeDeviceOrientationOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "SubscribeDeviceOrientationOptions",
        )),
        ("@kit.SensorServiceKit", "SubscribeGyroscopeOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "SubscribeGyroscopeOptions",
        )),
        ("@kit.SensorServiceKit", "SubscribeHeartRateOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "SubscribeHeartRateOptions",
        )),
        ("@kit.SensorServiceKit", "SubscribeLightOptions") => {
            Some(KitEntry::Named("@system:sensor", "SubscribeLightOptions"))
        }
        ("@kit.SensorServiceKit", "SubscribeOnBodyStateOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "SubscribeOnBodyStateOptions",
        )),
        ("@kit.SensorServiceKit", "SubscribeProximityOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "SubscribeProximityOptions",
        )),
        ("@kit.SensorServiceKit", "SubscribeStepCounterOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "SubscribeStepCounterOptions",
        )),
        ("@kit.SensorServiceKit", "VibrateOptions") => {
            Some(KitEntry::Named("@system:vibrator", "VibrateOptions"))
        }
        ("@kit.SensorServiceKit", "Vibrator") => Some(KitEntry::Default("@system:vibrator")),
        ("@kit.SensorServiceKit", "sensor") => Some(KitEntry::Default("@ohos:sensor")),
        ("@kit.SensorServiceKit", "subscribeAccelerometerOptions") => Some(KitEntry::Named(
            "@system:sensor",
            "subscribeAccelerometerOptions",
        )),
        ("@kit.SensorServiceKit", "vibrator") => Some(KitEntry::Default("@ohos:vibrator")),

        // ── @kit.TelephonyKit ───────────────────────────────────────────────
        ("@kit.TelephonyKit", "call") => Some(KitEntry::Default("@ohos:telephony.call")),
        ("@kit.TelephonyKit", "data") => Some(KitEntry::Default("@ohos:telephony.data")),
        ("@kit.TelephonyKit", "eSIM") => Some(KitEntry::Default("@ohos:telephony.esim")),
        ("@kit.TelephonyKit", "observer") => Some(KitEntry::Default("@ohos:telephony.observer")),
        ("@kit.TelephonyKit", "radio") => Some(KitEntry::Default("@ohos:telephony.radio")),
        ("@kit.TelephonyKit", "sim") => Some(KitEntry::Default("@ohos:telephony.sim")),
        ("@kit.TelephonyKit", "sms") => Some(KitEntry::Default("@ohos:telephony.sms")),
        ("@kit.TelephonyKit", "vcard") => Some(KitEntry::Default("@ohos:telephony.vcard")),

        // ── @kit.TestKit ───────────────────────────────────────────────
        ("@kit.TestKit", "BY") => Some(KitEntry::Named("@ohos:UiTest", "BY")),
        ("@kit.TestKit", "By") => Some(KitEntry::Named("@ohos:UiTest", "By")),
        ("@kit.TestKit", "Component") => Some(KitEntry::Named("@ohos:UiTest", "Component")),
        ("@kit.TestKit", "DisplayRotation") => {
            Some(KitEntry::Named("@ohos:UiTest", "DisplayRotation"))
        }
        ("@kit.TestKit", "Driver") => Some(KitEntry::Named("@ohos:UiTest", "Driver")),
        ("@kit.TestKit", "InputTextMode") => Some(KitEntry::Named("@ohos:UiTest", "InputTextMode")),
        ("@kit.TestKit", "MatchPattern") => Some(KitEntry::Named("@ohos:UiTest", "MatchPattern")),
        ("@kit.TestKit", "MouseButton") => Some(KitEntry::Named("@ohos:UiTest", "MouseButton")),
        ("@kit.TestKit", "ON") => Some(KitEntry::Named("@ohos:UiTest", "ON")),
        ("@kit.TestKit", "On") => Some(KitEntry::Named("@ohos:UiTest", "On")),
        ("@kit.TestKit", "PerfMeasureResult") => {
            Some(KitEntry::Named("@ohos:test.PerfTest", "PerfMeasureResult"))
        }
        ("@kit.TestKit", "PerfMetric") => {
            Some(KitEntry::Named("@ohos:test.PerfTest", "PerfMetric"))
        }
        ("@kit.TestKit", "PerfTest") => Some(KitEntry::Named("@ohos:test.PerfTest", "PerfTest")),
        ("@kit.TestKit", "PerfTestStrategy") => {
            Some(KitEntry::Named("@ohos:test.PerfTest", "PerfTestStrategy"))
        }
        ("@kit.TestKit", "Point") => Some(KitEntry::Named("@ohos:UiTest", "Point")),
        ("@kit.TestKit", "PointerMatrix") => Some(KitEntry::Named("@ohos:UiTest", "PointerMatrix")),
        ("@kit.TestKit", "Rect") => Some(KitEntry::Named("@ohos:UiTest", "Rect")),
        ("@kit.TestKit", "ResizeDirection") => {
            Some(KitEntry::Named("@ohos:UiTest", "ResizeDirection"))
        }
        ("@kit.TestKit", "TestRunner") => Some(KitEntry::Default("@ohos:application.testRunner")),
        ("@kit.TestKit", "TouchPadSwipeOptions") => {
            Some(KitEntry::Named("@ohos:UiTest", "TouchPadSwipeOptions"))
        }
        ("@kit.TestKit", "UIElementInfo") => Some(KitEntry::Named("@ohos:UiTest", "UIElementInfo")),
        ("@kit.TestKit", "UIEventObserver") => {
            Some(KitEntry::Named("@ohos:UiTest", "UIEventObserver"))
        }
        ("@kit.TestKit", "UiComponent") => Some(KitEntry::Named("@ohos:UiTest", "UiComponent")),
        ("@kit.TestKit", "UiDirection") => Some(KitEntry::Named("@ohos:UiTest", "UiDirection")),
        ("@kit.TestKit", "UiDriver") => Some(KitEntry::Named("@ohos:UiTest", "UiDriver")),
        ("@kit.TestKit", "UiWindow") => Some(KitEntry::Named("@ohos:UiTest", "UiWindow")),
        ("@kit.TestKit", "WindowFilter") => Some(KitEntry::Named("@ohos:UiTest", "WindowFilter")),
        ("@kit.TestKit", "WindowMode") => Some(KitEntry::Named("@ohos:UiTest", "WindowMode")),
        ("@kit.TestKit", "abilityDelegatorRegistry") => Some(KitEntry::Default(
            "@ohos:app.ability.abilityDelegatorRegistry",
        )),

        // ── @kit.UniversalKeystoreKit ───────────────────────────────────────────────
        ("@kit.UniversalKeystoreKit", "huks") => Some(KitEntry::Default("@ohos:security.huks")),

        // ── @kit.UserAuthenticationKit ───────────────────────────────────────────────
        ("@kit.UserAuthenticationKit", "UserAuthIcon") => {
            Some(KitEntry::Default("@ohos:userIAM.userAuthIcon"))
        }
        ("@kit.UserAuthenticationKit", "faceAuth") => {
            Some(KitEntry::Default("@ohos:userIAM.faceAuth"))
        }
        ("@kit.UserAuthenticationKit", "userAccessCtrl") => {
            Some(KitEntry::Default("@ohos:userIAM.userAccessCtrl"))
        }
        ("@kit.UserAuthenticationKit", "userAuth") => {
            Some(KitEntry::Default("@ohos:userIAM.userAuth"))
        }

        // ── @kit.AREngine (HMS) ───────────────────────────────────────
        ("@kit.AREngine", "ARView") => Some(KitEntry::Named("@hms:core.ar.arview", "ARView")),
        ("@kit.AREngine", "arEngine") => Some(KitEntry::Default("@hms:core.ar.arengine")),
        ("@kit.AREngine", "arViewController") => {
            Some(KitEntry::Named("@hms:core.ar.arview", "arViewController"))
        }

        // ── @kit.AccountKit (HMS) ───────────────────────────────────────
        ("@kit.AccountKit", "LoginPanel") => Some(KitEntry::Named(
            "@hms:core.account.LoginComponent",
            "LoginPanel",
        )),
        ("@kit.AccountKit", "LoginWithHuaweiIDButton") => Some(KitEntry::Named(
            "@hms:core.account.LoginComponent",
            "LoginWithHuaweiIDButton",
        )),
        ("@kit.AccountKit", "authentication") => {
            Some(KitEntry::Default("@hms:core.authentication"))
        }
        ("@kit.AccountKit", "extendService") => {
            Some(KitEntry::Default("@hms:core.account.extendservice"))
        }
        ("@kit.AccountKit", "invoiceAssistant") => {
            Some(KitEntry::Default("@hms:core.account.invoiceAssistant"))
        }
        ("@kit.AccountKit", "loginComponentManager") => Some(KitEntry::Named(
            "@hms:core.account.LoginComponent",
            "loginComponentManager",
        )),
        ("@kit.AccountKit", "minorsProtection") => {
            Some(KitEntry::Default("@hms:core.account.minorsProtection"))
        }
        ("@kit.AccountKit", "realName") => Some(KitEntry::Default("@hms:core.account.realname")),
        ("@kit.AccountKit", "shippingAddress") => {
            Some(KitEntry::Default("@hms:core.account.shippingAddress"))
        }

        // ── @kit.AgentFrameworkKit (HMS) ───────────────────────────────────────
        ("@kit.AgentFrameworkKit", "AgentController") => {
            Some(KitEntry::Named("@hms:ai.AgentFramework", "AgentController"))
        }
        ("@kit.AgentFrameworkKit", "BaseOptions") => {
            Some(KitEntry::Named("@hms:ai.AgentFramework", "BaseOptions"))
        }
        ("@kit.AgentFrameworkKit", "ButtonType") => {
            Some(KitEntry::Named("@hms:ai.AgentFramework", "ButtonType"))
        }
        ("@kit.AgentFrameworkKit", "FunctionComponent") => Some(KitEntry::Named(
            "@hms:ai.AgentFramework",
            "FunctionComponent",
        )),
        ("@kit.AgentFrameworkKit", "FunctionController") => Some(KitEntry::Named(
            "@hms:ai.AgentFramework",
            "FunctionController",
        )),
        ("@kit.AgentFrameworkKit", "FunctionOptions") => {
            Some(KitEntry::Named("@hms:ai.AgentFramework", "FunctionOptions"))
        }

        // ── @kit.AppGalleryKit (HMS) ───────────────────────────────────────
        ("@kit.AppGalleryKit", "appInfoManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.appInfoManager",
        )),
        ("@kit.AppGalleryKit", "attributionManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.attributionManager",
        )),
        ("@kit.AppGalleryKit", "attributionTestManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.attributionTestManager",
        )),
        ("@kit.AppGalleryKit", "commentManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.commentManager",
        )),
        ("@kit.AppGalleryKit", "moduleInstallManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.moduleInstallManager",
        )),
        ("@kit.AppGalleryKit", "privacyManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.privacyManager",
        )),
        ("@kit.AppGalleryKit", "productViewManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.productViewManager",
        )),
        ("@kit.AppGalleryKit", "sceneManager") => {
            Some(KitEntry::Default("@hms:bundle.sceneManager"))
        }
        ("@kit.AppGalleryKit", "updateManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.updateManager",
        )),

        // ── @kit.AppLinkingKit (HMS) ───────────────────────────────────────
        ("@kit.AppLinkingKit", "deferredLink") => {
            Some(KitEntry::Default("@hms:bundle.applinking.deferredLink"))
        }

        // ── @kit.CallKit (HMS) ───────────────────────────────────────
        ("@kit.CallKit", "voipCall") => Some(KitEntry::Default("@hms:telephony.voipCall")),

        // ── @kit.CallServiceKit (HMS) ───────────────────────────────────────
        ("@kit.CallServiceKit", "CallerInfo") => Some(KitEntry::Named(
            "@hms:telephony.CallerInfoQueryExtensionAbility",
            "CallerInfo",
        )),
        ("@kit.CallServiceKit", "CallerInfoQueryExtensionAbility") => Some(KitEntry::Default(
            "@hms:telephony.CallerInfoQueryExtensionAbility",
        )),
        ("@kit.CallServiceKit", "CallerInfoQueryExtensionContext") => Some(KitEntry::Default(
            "@hms:telephony.CallerInfoQueryExtensionContext",
        )),
        ("@kit.CallServiceKit", "voipCall") => Some(KitEntry::Default("@hms:telephony.voipCall")),

        // ── @kit.CarKit (HMS) ───────────────────────────────────────
        ("@kit.CarKit", "navigationInfoMgr") => {
            Some(KitEntry::Default("@hms:carService.navigationInfoMgr"))
        }
        ("@kit.CarKit", "smartMobilityCommon") => {
            Some(KitEntry::Default("@hms:carService.smartMobilityCommon"))
        }

        // ── @kit.CloudFoundationKit (HMS) ───────────────────────────────────────
        ("@kit.CloudFoundationKit", "cloudCommon") => Some(KitEntry::Default(
            "@hms:core.deviceCloudGateway.cloudCommon",
        )),
        ("@kit.CloudFoundationKit", "cloudDatabase") => Some(KitEntry::Default(
            "@hms:core.deviceCloudGateway.cloudDatabase",
        )),
        ("@kit.CloudFoundationKit", "cloudFunction") => Some(KitEntry::Default(
            "@hms:core.deviceCloudGateway.cloudFunction",
        )),
        ("@kit.CloudFoundationKit", "cloudResPrefetch") => Some(KitEntry::Default(
            "@hms:core.deviceCloudGateway.cloudResPrefetch",
        )),
        ("@kit.CloudFoundationKit", "cloudStorage") => Some(KitEntry::Default(
            "@hms:core.deviceCloudGateway.cloudStorage",
        )),

        // ── @kit.CoreSpeechKit (HMS) ───────────────────────────────────────
        ("@kit.CoreSpeechKit", "speechRecognizer") => {
            Some(KitEntry::Default("@hms:ai.speechRecognizer"))
        }
        ("@kit.CoreSpeechKit", "textToSpeech") => Some(KitEntry::Default("@hms:ai.textToSpeech")),

        // ── @kit.CoreVisionKit (HMS) ───────────────────────────────────────
        ("@kit.CoreVisionKit", "faceComparator") => {
            Some(KitEntry::Default("@hms:ai.face.faceComparator"))
        }
        ("@kit.CoreVisionKit", "faceDetector") => {
            Some(KitEntry::Default("@hms:ai.face.faceDetector"))
        }
        ("@kit.CoreVisionKit", "objectDetection") => {
            Some(KitEntry::Default("@hms:ai.vision.objectDetection"))
        }
        ("@kit.CoreVisionKit", "skeletonDetection") => {
            Some(KitEntry::Default("@hms:ai.vision.skeletonDetection"))
        }
        ("@kit.CoreVisionKit", "subjectSegmentation") => {
            Some(KitEntry::Default("@hms:ai.vision.subjectSegmentation"))
        }
        ("@kit.CoreVisionKit", "textRecognition") => {
            Some(KitEntry::Default("@hms:ai.ocr.textRecognition"))
        }
        ("@kit.CoreVisionKit", "visionBase") => {
            Some(KitEntry::Default("@hms:ai.vision.visionBase"))
        }

        // ── @kit.DataAugmentationKit (HMS) ───────────────────────────────────────
        ("@kit.DataAugmentationKit", "knowledgeProcessor") => {
            Some(KitEntry::Default("@hms:data.knowledgeProcessor"))
        }
        ("@kit.DataAugmentationKit", "localChatModel") => {
            Some(KitEntry::Default("@hms:data.localChatModel"))
        }
        ("@kit.DataAugmentationKit", "rag") => Some(KitEntry::Default("@hms:data.rag")),
        ("@kit.DataAugmentationKit", "retrieval") => Some(KitEntry::Default("@hms:data.retrieval")),

        // ── @kit.DeskTopExtensionKit (HMS) ───────────────────────────────────────
        ("@kit.DeskTopExtensionKit", "StatusBarViewExtensionAbility") => Some(KitEntry::Default(
            "@hms:pcService.StatusBarViewExtensionAbility",
        )),
        ("@kit.DeskTopExtensionKit", "statusBarManager") => {
            Some(KitEntry::Default("@hms:pcService.statusBarManager"))
        }

        // ── @kit.DeviceSecurityKit (HMS) ───────────────────────────────────────
        ("@kit.DeviceSecurityKit", "antifraudPicker") => {
            Some(KitEntry::Default("@hms:security.antifraudPicker"))
        }
        ("@kit.DeviceSecurityKit", "businessRiskIntelligentDetection") => Some(KitEntry::Default(
            "@hms:security.businessRiskIntelligentDetection",
        )),
        ("@kit.DeviceSecurityKit", "deviceCertificate") => {
            Some(KitEntry::Default("@hms:security.deviceCertificate"))
        }
        ("@kit.DeviceSecurityKit", "dlpAntiPeep") => {
            Some(KitEntry::Default("@hms:security.dlpAntiPeep"))
        }
        ("@kit.DeviceSecurityKit", "safetyDetect") => {
            Some(KitEntry::Default("@hms:security.safetyDetect"))
        }
        ("@kit.DeviceSecurityKit", "securityAudit") => {
            Some(KitEntry::Default("@hms:security.securityAudit"))
        }
        ("@kit.DeviceSecurityKit", "trustedAppService") => {
            Some(KitEntry::Default("@hms:security.trustedAppService"))
        }
        ("@kit.DeviceSecurityKit", "trustedAuthentication") => {
            Some(KitEntry::Default("@hms:security.trustedAuthentication"))
        }

        // ── @kit.EnterpriseDataGuardKit (HMS) ───────────────────────────────────────
        ("@kit.EnterpriseDataGuardKit", "fileGuard") => {
            Some(KitEntry::Default("@hms:pcService.fileGuard"))
        }
        ("@kit.EnterpriseDataGuardKit", "recoveryKey") => {
            Some(KitEntry::Default("@hms:pcService.recoveryKeyService"))
        }

        // ── @kit.EnterpriseSpaceKit (HMS) ───────────────────────────────────────
        ("@kit.EnterpriseSpaceKit", "fileTransfer") => Some(KitEntry::Default(
            "@hms:enterpriseSpaceService.fileTransfer",
        )),
        ("@kit.EnterpriseSpaceKit", "spaceManager") => Some(KitEntry::Default(
            "@hms:enterpriseSpaceService.spaceManager",
        )),

        // ── @kit.FileManagerServiceKit (HMS) ───────────────────────────────────────
        ("@kit.FileManagerServiceKit", "fileManagerService") => {
            Some(KitEntry::Default("@hms:filemanagement.fileManagerService"))
        }

        // ── @kit.GameServiceKit (HMS) ───────────────────────────────────────
        ("@kit.GameServiceKit", "gameNearbyTransfer") => Some(KitEntry::Default(
            "@hms:core.gameservice.gamenearbytransfer",
        )),
        ("@kit.GameServiceKit", "gamePerformance") => {
            Some(KitEntry::Default("@hms:core.gameservice.gameperformance"))
        }
        ("@kit.GameServiceKit", "gamePlayer") => {
            Some(KitEntry::Default("@hms:core.gameservice.gameplayer"))
        }

        // ── @kit.GraphicsAccelerateKit (HMS) ───────────────────────────────────────
        ("@kit.GraphicsAccelerateKit", "AssetAccelerationExtensionAbility") => Some(
            KitEntry::Default("@hms:gameAcceleration.AssetAccelerationExtensionAbility"),
        ),
        ("@kit.GraphicsAccelerateKit", "AssetAccelerationExtensionContext") => Some(
            KitEntry::Default("@hms:gameAcceleration.AssetAccelerationExtensionContext"),
        ),
        ("@kit.GraphicsAccelerateKit", "AssetAccelerationExtensionInfo") => Some(KitEntry::Named(
            "@hms:gameAcceleration.AssetAccelerationExtensionAbility",
            "AssetAccelerationExtensionInfo",
        )),
        ("@kit.GraphicsAccelerateKit", "ContentRequestType") => Some(KitEntry::Named(
            "@hms:gameAcceleration.AssetAccelerationExtensionAbility",
            "ContentRequestType",
        )),
        ("@kit.GraphicsAccelerateKit", "assetDownloadManager") => Some(KitEntry::Default(
            "@hms:gameAcceleration.assetDownloadManager",
        )),
        ("@kit.GraphicsAccelerateKit", "launchAcceleration") => Some(KitEntry::Default(
            "@hms:gameAcceleration.launchAcceleration",
        )),

        // ── @kit.HealthServiceKit (HMS) ───────────────────────────────────────
        ("@kit.HealthServiceKit", "healthService") => {
            Some(KitEntry::Default("@hms:health.service"))
        }
        ("@kit.HealthServiceKit", "healthStore") => Some(KitEntry::Default("@hms:health.store")),

        // ── @kit.IAPKit (HMS) ───────────────────────────────────────
        ("@kit.IAPKit", "iap") => Some(KitEntry::Default("@hms:core.iap")),

        // ── @kit.IntentsKit (HMS) ───────────────────────────────────────
        ("@kit.IntentsKit", "InsightIntentUIExtensionAbility") => Some(KitEntry::Default(
            "@hms:ai.insightIntent.InsightIntentUIExtensionAbility",
        )),
        ("@kit.IntentsKit", "insightIntent") => Some(KitEntry::Default("@hms:ai.insightIntent")),

        // ── @kit.LiveViewKit (HMS) ───────────────────────────────────────
        ("@kit.LiveViewKit", "LiveViewLockScreenExtensionAbility") => Some(KitEntry::Default(
            "@hms:core.liveview.LiveViewLockScreenExtensionAbility",
        )),
        ("@kit.LiveViewKit", "LiveViewLockScreenExtensionContext") => Some(KitEntry::Default(
            "@hms:core.liveview.LiveViewLockScreenExtensionContext",
        )),
        ("@kit.LiveViewKit", "liveViewManager") => {
            Some(KitEntry::Default("@hms:core.liveview.liveViewManager"))
        }

        // ── @kit.MapKit (HMS) ───────────────────────────────────────
        ("@kit.MapKit", "MapComponent") => Some(KitEntry::Named(
            "@hms:core.map.MapComponent",
            "MapComponent",
        )),
        ("@kit.MapKit", "map") => Some(KitEntry::Default("@hms:core.map.map")),
        ("@kit.MapKit", "mapCommon") => Some(KitEntry::Default("@hms:core.map.mapCommon")),
        ("@kit.MapKit", "navi") => Some(KitEntry::Default("@hms:core.map.navi")),
        ("@kit.MapKit", "petalMaps") => Some(KitEntry::Default("@hms:core.map.petalMaps")),
        ("@kit.MapKit", "sceneMap") => Some(KitEntry::Default("@hms:core.map.sceneMap")),
        ("@kit.MapKit", "site") => Some(KitEntry::Default("@hms:core.map.site")),
        ("@kit.MapKit", "staticMap") => Some(KitEntry::Default("@hms:core.map.staticMap")),

        // ── @kit.NaturalLanguageKit (HMS) ───────────────────────────────────────
        ("@kit.NaturalLanguageKit", "EntityType") => {
            Some(KitEntry::Named("@hms:ai.nlp.textProcessing", "EntityType"))
        }
        ("@kit.NaturalLanguageKit", "textProcessing") => {
            Some(KitEntry::Default("@hms:ai.nlp.textProcessing"))
        }

        // ── @kit.NearLinkKit (HMS) ───────────────────────────────────────
        ("@kit.NearLinkKit", "advertising") => Some(KitEntry::Default("@hms:nearlink.advertising")),
        ("@kit.NearLinkKit", "constant") => Some(KitEntry::Default("@hms:nearlink.constant")),
        ("@kit.NearLinkKit", "dataTransfer") => {
            Some(KitEntry::Default("@hms:nearlink.dataTransfer"))
        }
        ("@kit.NearLinkKit", "manager") => Some(KitEntry::Default("@hms:nearlink.manager")),
        ("@kit.NearLinkKit", "remoteDevice") => {
            Some(KitEntry::Default("@hms:nearlink.remoteDevice"))
        }
        ("@kit.NearLinkKit", "scan") => Some(KitEntry::Default("@hms:nearlink.scan")),
        ("@kit.NearLinkKit", "ssap") => Some(KitEntry::Default("@hms:nearlink.ssap")),

        // ── @kit.NetworkBoostKit (HMS) ───────────────────────────────────────
        ("@kit.NetworkBoostKit", "netBoost") => {
            Some(KitEntry::Default("@hms:networkboost.netBoost"))
        }
        ("@kit.NetworkBoostKit", "netHandover") => {
            Some(KitEntry::Default("@hms:networkboost.handover"))
        }
        ("@kit.NetworkBoostKit", "netQuality") => {
            Some(KitEntry::Default("@hms:networkboost.netquality"))
        }

        // ── @kit.OnlineAuthenticationKit (HMS) ───────────────────────────────────────
        ("@kit.OnlineAuthenticationKit", "fido") => Some(KitEntry::Default("@hms:security.fido")),
        ("@kit.OnlineAuthenticationKit", "fido2") => Some(KitEntry::Default("@hms:security.fido2")),
        ("@kit.OnlineAuthenticationKit", "ifaa") => Some(KitEntry::Default("@hms:security.ifaa")),
        ("@kit.OnlineAuthenticationKit", "soter") => Some(KitEntry::Default("@hms:security.soter")),

        // ── @kit.PDFKit (HMS) ───────────────────────────────────────
        ("@kit.PDFKit", "PdfView") => {
            Some(KitEntry::Named("@hms:officeservice.PdfView", "PdfView"))
        }
        ("@kit.PDFKit", "pdfService") => Some(KitEntry::Default("@hms:officeservice.pdfservice")),
        ("@kit.PDFKit", "pdfViewManager") => Some(KitEntry::Named(
            "@hms:officeservice.PdfView",
            "pdfViewManager",
        )),

        // ── @kit.PaymentKit (HMS) ───────────────────────────────────────
        ("@kit.PaymentKit", "ecnyPaymentService") => {
            Some(KitEntry::Default("@hms:core.payment.ecnyPaymentService"))
        }
        ("@kit.PaymentKit", "paymentService") => {
            Some(KitEntry::Default("@hms:core.payment.paymentService"))
        }
        ("@kit.PaymentKit", "realNameService") => {
            Some(KitEntry::Default("@hms:core.payment.realNameService"))
        }
        ("@kit.PaymentKit", "thirdPaymentService") => {
            Some(KitEntry::Default("@hms:core.payment.thirdPaymentService"))
        }

        // ── @kit.Penkit (HMS) ───────────────────────────────────────
        ("@kit.Penkit", "HandwriteComponent") => Some(KitEntry::Named(
            "@hms:stylus.HandwriteComponent",
            "HandwriteComponent",
        )),
        ("@kit.Penkit", "HandwriteController") => Some(KitEntry::Named(
            "@hms:stylus.HandwriteComponent",
            "HandwriteController",
        )),
        ("@kit.Penkit", "InstantShapeGenerator") => Some(KitEntry::Named(
            "@hms:stylus.handwrite",
            "InstantShapeGenerator",
        )),
        ("@kit.Penkit", "PenHspInfo") => Some(KitEntry::Named(
            "@hms:stylus.HandwriteComponent",
            "PenHspInfo",
        )),
        ("@kit.Penkit", "PenType") => {
            Some(KitEntry::Named("@hms:stylus.HandwriteComponent", "PenType"))
        }
        ("@kit.Penkit", "PointPredictor") => {
            Some(KitEntry::Named("@hms:stylus.handwrite", "PointPredictor"))
        }
        ("@kit.Penkit", "ShapeInfo") => Some(KitEntry::Named("@hms:stylus.handwrite", "ShapeInfo")),
        ("@kit.Penkit", "imageFeaturePicker") => {
            Some(KitEntry::Default("@hms:officeservice.imageFeaturePicker"))
        }
        ("@kit.Penkit", "stylusInteraction") => {
            Some(KitEntry::Default("@hms:officeservice.stylusInteraction"))
        }

        // ── @kit.PreviewKit (HMS) ───────────────────────────────────────
        ("@kit.PreviewKit", "filePreview") => {
            Some(KitEntry::Default("@hms:filemanagement.filepreview"))
        }
        ("@kit.PreviewKit", "openFileBoost") => {
            Some(KitEntry::Default("@hms:pcService.openFileBoost"))
        }

        // ── @kit.PushKit (HMS) ───────────────────────────────────────
        ("@kit.PushKit", "AAID") => Some(KitEntry::Default("@hms:core.AAID")),
        ("@kit.PushKit", "PushExtensionAbility") => {
            Some(KitEntry::Default("@hms:core.push.PushExtensionAbility"))
        }
        ("@kit.PushKit", "PushExtensionContext") => {
            Some(KitEntry::Default("@hms:core.push.PushExtensionContext"))
        }
        ("@kit.PushKit", "RemoteLocationExtensionAbility") => Some(KitEntry::Default(
            "@hms:core.push.RemoteLocationExtensionAbility",
        )),
        ("@kit.PushKit", "RemoteLocationExtensionContext") => Some(KitEntry::Default(
            "@hms:core.push.RemoteLocationExtensionContext",
        )),
        ("@kit.PushKit", "RemoteNotificationExtensionAbility") => Some(KitEntry::Default(
            "@hms:core.push.RemoteNotificationExtensionAbility",
        )),
        ("@kit.PushKit", "RemoteNotificationExtensionContext") => Some(KitEntry::Default(
            "@hms:core.push.RemoteNotificationExtensionContext",
        )),
        ("@kit.PushKit", "VoIPExtensionAbility") => {
            Some(KitEntry::Default("@hms:core.push.VoIPExtensionAbility"))
        }
        ("@kit.PushKit", "VoIPExtensionContext") => {
            Some(KitEntry::Default("@hms:core.push.VoIPExtensionContext"))
        }
        ("@kit.PushKit", "pushCommon") => Some(KitEntry::Default("@hms:core.push.pushCommon")),
        ("@kit.PushKit", "pushService") => Some(KitEntry::Default("@hms:core.push.pushService")),
        ("@kit.PushKit", "serviceNotification") => {
            Some(KitEntry::Default("@hms:core.push.serviceNotification"))
        }

        // ── @kit.ReaderKit (HMS) ───────────────────────────────────────
        ("@kit.ReaderKit", "ReadPageComponent") => Some(KitEntry::Named(
            "@hms:core.readerservice.readerComponent",
            "ReadPageComponent",
        )),
        ("@kit.ReaderKit", "bookParser") => Some(KitEntry::Named(
            "@hms:core.readerservice.bookParser",
            "bookParser",
        )),
        ("@kit.ReaderKit", "readerCore") => Some(KitEntry::Named(
            "@hms:core.readerservice.readerComponent",
            "readerCore",
        )),

        // ── @kit.RemoteCommunicationKit (HMS) ───────────────────────────────────────
        ("@kit.RemoteCommunicationKit", "rcp") => Some(KitEntry::Default("@hms:collaboration.rcp")),
        ("@kit.RemoteCommunicationKit", "urpc") => {
            Some(KitEntry::Default("@hms:collaboration.urpc"))
        }

        // ── @kit.RingtoneKit (HMS) ───────────────────────────────────────
        ("@kit.RingtoneKit", "ringtone") => Some(KitEntry::Default("@hms:core.ringtone")),

        // ── @kit.ScanKit (HMS) ───────────────────────────────────────
        ("@kit.ScanKit", "customScan") => Some(KitEntry::Default("@hms:core.scan.customScan")),
        ("@kit.ScanKit", "detectBarcode") => {
            Some(KitEntry::Default("@hms:core.scan.detectBarcode"))
        }
        ("@kit.ScanKit", "generateBarcode") => {
            Some(KitEntry::Default("@hms:core.scan.generateBarcode"))
        }
        ("@kit.ScanKit", "scanBarcode") => Some(KitEntry::Default("@hms:core.scan.scanBarcode")),
        ("@kit.ScanKit", "scanCore") => Some(KitEntry::Default("@hms:core.scan.scanCore")),

        // ── @kit.ScenarioFusionKit (HMS) ───────────────────────────────────────
        ("@kit.ScenarioFusionKit", "FunctionalButton") => Some(KitEntry::Named(
            "@hms:core.atomicserviceComponent.atomicserviceUi",
            "FunctionalButton",
        )),
        ("@kit.ScenarioFusionKit", "FunctionalInput") => Some(KitEntry::Named(
            "@hms:core.atomicserviceComponent.atomicserviceInput",
            "FunctionalInput",
        )),
        ("@kit.ScenarioFusionKit", "atomicService") => Some(KitEntry::Default(
            "@hms:core.atomicserviceComponent.atomicservice",
        )),
        ("@kit.ScenarioFusionKit", "fileUriService") => Some(KitEntry::Default(
            "@hms:core.scenarioFusionComponent.fileUriService",
        )),
        ("@kit.ScenarioFusionKit", "functionalButtonComponentManager") => Some(KitEntry::Named(
            "@hms:core.atomicserviceComponent.atomicserviceUi",
            "functionalButtonComponentManager",
        )),
        ("@kit.ScenarioFusionKit", "functionalInputComponentManager") => Some(KitEntry::Named(
            "@hms:core.atomicserviceComponent.atomicserviceInput",
            "functionalInputComponentManager",
        )),

        // ── @kit.ScreenTimeGuardKit (HMS) ───────────────────────────────────────
        ("@kit.ScreenTimeGuardKit", "TimeGuardExtensionAbility") => Some(KitEntry::Default(
            "@hms:utilityApplication.screenTimeGuard.TimeGuardExtensionAbility",
        )),
        ("@kit.ScreenTimeGuardKit", "TimeGuardExtensionContext") => Some(KitEntry::Default(
            "@hms:utilityApplication.screenTimeGuard.TimeGuardExtensionContext",
        )),
        ("@kit.ScreenTimeGuardKit", "appPicker") => Some(KitEntry::Default(
            "@hms:utilityApplication.screenTimeGuard.appPicker",
        )),
        ("@kit.ScreenTimeGuardKit", "guardService") => Some(KitEntry::Default(
            "@hms:utilityApplication.screenTimeGuard.guardService",
        )),

        // ── @kit.ServiceCollaborationKit (HMS) ───────────────────────────────────────
        ("@kit.ServiceCollaborationKit", "CollaborationCameraBusinessFilter") => {
            Some(KitEntry::Named(
                "@hms:collaboration.camera",
                "CollaborationCameraBusinessFilter",
            ))
        }
        ("@kit.ServiceCollaborationKit", "CollaborationCameraStateDialog") => {
            Some(KitEntry::Named(
                "@hms:collaboration.camera",
                "CollaborationCameraStateDialog",
            ))
        }
        ("@kit.ServiceCollaborationKit", "CollaborationDevicePicker") => Some(KitEntry::Named(
            "@hms:collaboration.CollaborationDevicePicker",
            "CollaborationDevicePicker",
        )),
        ("@kit.ServiceCollaborationKit", "CollaborationServiceFilter") => Some(KitEntry::Named(
            "@hms:collaboration.service",
            "CollaborationServiceFilter",
        )),
        ("@kit.ServiceCollaborationKit", "CollaborationServiceStateDialog") => {
            Some(KitEntry::Named(
                "@hms:collaboration.service",
                "CollaborationServiceStateDialog",
            ))
        }
        ("@kit.ServiceCollaborationKit", "createCollaborationCameraMenuItems") => {
            Some(KitEntry::Named(
                "@hms:collaboration.camera",
                "createCollaborationCameraMenuItems",
            ))
        }
        ("@kit.ServiceCollaborationKit", "createCollaborationServiceMenuItems") => {
            Some(KitEntry::Named(
                "@hms:collaboration.service",
                "createCollaborationServiceMenuItems",
            ))
        }
        ("@kit.ServiceCollaborationKit", "devicePicker") => {
            Some(KitEntry::Default("@hms:collaboration.devicePicker"))
        }

        // ── @kit.ShareKit (HMS) ───────────────────────────────────────
        ("@kit.ShareKit", "harmonyShare") => {
            Some(KitEntry::Default("@hms:collaboration.harmonyShare"))
        }
        ("@kit.ShareKit", "systemShare") => {
            Some(KitEntry::Default("@hms:collaboration.systemShare"))
        }

        // ── @kit.SpatialReconKit (HMS) ───────────────────────────────────────
        ("@kit.SpatialReconKit", "spatialRender") => {
            Some(KitEntry::Default("@hms:graphics.spatialRender"))
        }

        // ── @kit.SpeechKit (HMS) ───────────────────────────────────────
        ("@kit.SpeechKit", "AICaptionComponent") => {
            Some(KitEntry::Named("@hms:ai.AICaption", "AICaptionComponent"))
        }
        ("@kit.SpeechKit", "AICaptionController") => {
            Some(KitEntry::Named("@hms:ai.AICaption", "AICaptionController"))
        }
        ("@kit.SpeechKit", "AICaptionOptions") => {
            Some(KitEntry::Named("@hms:ai.AICaption", "AICaptionOptions"))
        }
        ("@kit.SpeechKit", "AudioData") => Some(KitEntry::Named("@hms:ai.AICaption", "AudioData")),
        ("@kit.SpeechKit", "AudioInfo") => Some(KitEntry::Named("@hms:ai.AICaption", "AudioInfo")),
        ("@kit.SpeechKit", "ReadStateCode") => {
            Some(KitEntry::Named("@hms:ai.textReader", "ReadStateCode"))
        }
        ("@kit.SpeechKit", "TextReader") => Some(KitEntry::Default("@hms:ai.textReader")),
        ("@kit.SpeechKit", "TextReaderIcon") => {
            Some(KitEntry::Named("@hms:ai.textReader", "TextReaderIcon"))
        }
        ("@kit.SpeechKit", "WindowManager") => {
            Some(KitEntry::Named("@hms:ai.textReader", "WindowManager"))
        }

        // ── @kit.StatusBarExtensionKit (HMS) ───────────────────────────────────────
        ("@kit.StatusBarExtensionKit", "StatusBarViewExtensionAbility") => Some(KitEntry::Default(
            "@hms:pcService.StatusBarViewExtensionAbility",
        )),
        ("@kit.StatusBarExtensionKit", "statusBarManager") => {
            Some(KitEntry::Default("@hms:pcService.statusBarManager"))
        }

        // ── @kit.StoreKit (HMS) ───────────────────────────────────────
        ("@kit.StoreKit", "appInfoManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.appInfoManager",
        )),
        ("@kit.StoreKit", "attributionManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.attributionManager",
        )),
        ("@kit.StoreKit", "attributionTestManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.attributionTestManager",
        )),
        ("@kit.StoreKit", "moduleInstallManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.moduleInstallManager",
        )),
        ("@kit.StoreKit", "privacyManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.privacyManager",
        )),
        ("@kit.StoreKit", "productViewManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.productViewManager",
        )),
        ("@kit.StoreKit", "sceneManager") => Some(KitEntry::Default("@hms:bundle.sceneManager")),
        ("@kit.StoreKit", "updateManager") => Some(KitEntry::Default(
            "@hms:core.appgalleryservice.updateManager",
        )),

        // ── @kit.UIDesignKit (HMS) ───────────────────────────────────────
        ("@kit.UIDesignKit", "AccessibilityOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "AccessibilityOptions",
        )),
        ("@kit.UIDesignKit", "ActionBarButton") => {
            Some(KitEntry::Named("@hms:hds.HdsActionBar", "ActionBarButton"))
        }
        ("@kit.UIDesignKit", "ActionBarButtonOptions") => Some(KitEntry::Named(
            "@hms:hds.HdsActionBar",
            "ActionBarButtonOptions",
        )),
        ("@kit.UIDesignKit", "ActionBarStyle") => {
            Some(KitEntry::Named("@hms:hds.HdsActionBar", "ActionBarStyle"))
        }
        ("@kit.UIDesignKit", "ActionBarStyleOptions") => Some(KitEntry::Named(
            "@hms:hds.HdsActionBar",
            "ActionBarStyleOptions",
        )),
        ("@kit.UIDesignKit", "BadgeOptions") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "BadgeOptions"))
        }
        ("@kit.UIDesignKit", "BlurStrategy") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "BlurStrategy"))
        }
        ("@kit.UIDesignKit", "BottomBuilderParams") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "BottomBuilderParams",
        )),
        ("@kit.UIDesignKit", "BottomBuilderShowType") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "BottomBuilderShowType",
        )),
        ("@kit.UIDesignKit", "ButtonOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "ButtonOptions",
        )),
        ("@kit.UIDesignKit", "CheckOptions") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "CheckOptions"))
        }
        ("@kit.UIDesignKit", "ColorOptions") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "ColorOptions"))
        }
        ("@kit.UIDesignKit", "CustomTransitionDelegate") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "CustomTransitionDelegate",
        )),
        ("@kit.UIDesignKit", "DeleteIconOptions") => {
            Some(KitEntry::Named("@hms:hds.HdsStyle", "DeleteIconOptions"))
        }
        ("@kit.UIDesignKit", "DividerMode") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "DividerMode"))
        }
        ("@kit.UIDesignKit", "DividerShowType") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "DividerShowType",
        )),
        ("@kit.UIDesignKit", "DualEdgeFlowLightWithMaskParam") => Some(KitEntry::Named(
            "@hms:hds.HdsVisualComponent",
            "DualEdgeFlowLightWithMaskParam",
        )),
        ("@kit.UIDesignKit", "DynamicHideParams") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "DynamicHideParams",
        )),
        ("@kit.UIDesignKit", "ExtendBarMode") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "ExtendBarMode",
        )),
        ("@kit.UIDesignKit", "FullDeleteOptions") => {
            Some(KitEntry::Named("@hms:hds.HdsStyle", "FullDeleteOptions"))
        }
        ("@kit.UIDesignKit", "HdsActionBar") => {
            Some(KitEntry::Named("@hms:hds.HdsActionBar", "HdsActionBar"))
        }
        ("@kit.UIDesignKit", "HdsBarMode") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "HdsBarMode"))
        }
        ("@kit.UIDesignKit", "HdsDividerStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsDividerStyle",
        )),
        ("@kit.UIDesignKit", "HdsListItem") => {
            Some(KitEntry::Named("@hms:hds.HdsStyle", "HdsListItem"))
        }
        ("@kit.UIDesignKit", "HdsListItemCard") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsListItemCard",
        )),
        ("@kit.UIDesignKit", "HdsListItemCardAttribute") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsListItemCardAttribute",
        )),
        ("@kit.UIDesignKit", "HdsListItemCardOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsListItemCardOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavDestination") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavDestination",
        )),
        ("@kit.UIDesignKit", "HdsNavDestinationAttribute") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavDestinationAttribute",
        )),
        ("@kit.UIDesignKit", "HdsNavDestinationTitleMode") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavDestinationTitleMode",
        )),
        ("@kit.UIDesignKit", "HdsNavigation") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigation",
        )),
        ("@kit.UIDesignKit", "HdsNavigationAttribute") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationAttribute",
        )),
        ("@kit.UIDesignKit", "HdsNavigationBackButtonItemOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationBackButtonItemOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavigationBackgroundStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationBackgroundStyle",
        )),
        ("@kit.UIDesignKit", "HdsNavigationBadgeIconOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationBadgeIconOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavigationBadgeOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationBadgeOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavigationDividerParams") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationDividerParams",
        )),
        ("@kit.UIDesignKit", "HdsNavigationDividerStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationDividerStyle",
        )),
        ("@kit.UIDesignKit", "HdsNavigationIconItemStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationIconItemStyle",
        )),
        ("@kit.UIDesignKit", "HdsNavigationIconOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationIconOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavigationMenuContentOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationMenuContentOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavigationMenuItemOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationMenuItemOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavigationTitle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationTitle",
        )),
        ("@kit.UIDesignKit", "HdsNavigationTitleBarOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationTitleBarOptions",
        )),
        ("@kit.UIDesignKit", "HdsNavigationTitleBarStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationTitleBarStyle",
        )),
        ("@kit.UIDesignKit", "HdsNavigationTitleMode") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationTitleMode",
        )),
        ("@kit.UIDesignKit", "HdsNavigationTitleStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsNavigationTitleStyle",
        )),
        ("@kit.UIDesignKit", "HdsSceneController") => Some(KitEntry::Named(
            "@hms:hds.HdsVisualComponent",
            "HdsSceneController",
        )),
        ("@kit.UIDesignKit", "HdsSceneType") => Some(KitEntry::Named(
            "@hms:hds.HdsVisualComponent",
            "HdsSceneType",
        )),
        ("@kit.UIDesignKit", "HdsSideBar") => {
            Some(KitEntry::Named("@hms:hds.HdsSideBar", "HdsSideBar"))
        }
        ("@kit.UIDesignKit", "HdsSideMenu") => {
            Some(KitEntry::Named("@hms:hds.HdsSideMenu", "HdsSideMenu"))
        }
        ("@kit.UIDesignKit", "HdsSideMenuBadgeParam") => Some(KitEntry::Named(
            "@hms:hds.HdsSideMenu",
            "HdsSideMenuBadgeParam",
        )),
        ("@kit.UIDesignKit", "HdsSideMenuBaseItemParam") => Some(KitEntry::Named(
            "@hms:hds.HdsSideMenu",
            "HdsSideMenuBaseItemParam",
        )),
        ("@kit.UIDesignKit", "HdsSideMenuMainItem") => Some(KitEntry::Named(
            "@hms:hds.HdsSideMenu",
            "HdsSideMenuMainItem",
        )),
        ("@kit.UIDesignKit", "HdsSideMenuMainItemParam") => Some(KitEntry::Named(
            "@hms:hds.HdsSideMenu",
            "HdsSideMenuMainItemParam",
        )),
        ("@kit.UIDesignKit", "HdsSideMenuSubItem") => Some(KitEntry::Named(
            "@hms:hds.HdsSideMenu",
            "HdsSideMenuSubItem",
        )),
        ("@kit.UIDesignKit", "HdsSideMenuSubItemParam") => Some(KitEntry::Named(
            "@hms:hds.HdsSideMenu",
            "HdsSideMenuSubItemParam",
        )),
        ("@kit.UIDesignKit", "HdsSnackBar") => {
            Some(KitEntry::Named("@hms:hds.HdsSnackBar", "HdsSnackBar"))
        }
        ("@kit.UIDesignKit", "HdsSwipeActionOptions") => Some(KitEntry::Named(
            "@hms:hds.HdsStyle",
            "HdsSwipeActionOptions",
        )),
        ("@kit.UIDesignKit", "HdsTabs") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "HdsTabs"))
        }
        ("@kit.UIDesignKit", "HdsTabsAttribute") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsTabsAttribute",
        )),
        ("@kit.UIDesignKit", "HdsTabsBackgroundStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsTabsBackgroundStyle",
        )),
        ("@kit.UIDesignKit", "HdsTabsController") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsTabsController",
        )),
        ("@kit.UIDesignKit", "HdsTitleBarContentStyle") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "HdsTitleBarContentStyle",
        )),
        ("@kit.UIDesignKit", "HdsVisualComponent") => Some(KitEntry::Named(
            "@hms:hds.HdsVisualComponent",
            "HdsVisualComponent",
        )),
        ("@kit.UIDesignKit", "HdsVisualComponentAttribute") => Some(KitEntry::Named(
            "@hms:hds.HdsVisualComponent",
            "HdsVisualComponentAttribute",
        )),
        ("@kit.UIDesignKit", "HideMode") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "HideMode"))
        }
        ("@kit.UIDesignKit", "IconOptions") => {
            Some(KitEntry::Named("@hms:hds.HdsStyle", "IconOptions"))
        }
        ("@kit.UIDesignKit", "IconSize") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "IconSize"))
        }
        ("@kit.UIDesignKit", "IconStyleMode") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "IconStyleMode",
        )),
        ("@kit.UIDesignKit", "IconType") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "IconType"))
        }
        ("@kit.UIDesignKit", "ImageClickOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "ImageClickOptions",
        )),
        ("@kit.UIDesignKit", "ImageOptions") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "ImageOptions"))
        }
        ("@kit.UIDesignKit", "ImageType") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "ImageType"))
        }
        ("@kit.UIDesignKit", "MultiWindowEntryInAPP") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "MultiWindowEntryInAPP",
        )),
        ("@kit.UIDesignKit", "MultiWindowEntryInAPPAttribute") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "MultiWindowEntryInAPPAttribute",
        )),
        ("@kit.UIDesignKit", "MultiWindowEntryInAPPIconOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "MultiWindowEntryInAPPIconOptions",
        )),
        ("@kit.UIDesignKit", "MultiWindowEntryInAPPMenuParams") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "MultiWindowEntryInAPPMenuParams",
        )),
        ("@kit.UIDesignKit", "MultiWindowEntryInAPPParams") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "MultiWindowEntryInAPPParams",
        )),
        ("@kit.UIDesignKit", "MultiWindowEntryInAPPSubtitleOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "MultiWindowEntryInAPPSubtitleOptions",
        )),
        ("@kit.UIDesignKit", "NavBarWidthRangeOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "NavBarWidthRangeOptions",
        )),
        ("@kit.UIDesignKit", "NavDestinationBuilder") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "NavDestinationBuilder",
        )),
        ("@kit.UIDesignKit", "NestedScrollInfo") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "NestedScrollInfo",
        )),
        ("@kit.UIDesignKit", "OnChangeCallback") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "OnChangeCallback",
        )),
        ("@kit.UIDesignKit", "OnClickCallback") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "OnClickCallback",
        )),
        ("@kit.UIDesignKit", "OnSelectCallback") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "OnSelectCallback",
        )),
        ("@kit.UIDesignKit", "PaddingOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "PaddingOptions",
        )),
        ("@kit.UIDesignKit", "PrefixBadge") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "PrefixBadge"))
        }
        ("@kit.UIDesignKit", "PrefixButton") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "PrefixButton"))
        }
        ("@kit.UIDesignKit", "PrefixCustomBuilder") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "PrefixCustomBuilder",
        )),
        ("@kit.UIDesignKit", "PrefixIcon") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "PrefixIcon"))
        }
        ("@kit.UIDesignKit", "PrefixIconOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "PrefixIconOptions",
        )),
        ("@kit.UIDesignKit", "PrefixIconType") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "PrefixIconType",
        )),
        ("@kit.UIDesignKit", "PrefixImage") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "PrefixImage"))
        }
        ("@kit.UIDesignKit", "PrefixSwitch") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "PrefixSwitch"))
        }
        ("@kit.UIDesignKit", "PrefixToggleButton") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "PrefixToggleButton",
        )),
        ("@kit.UIDesignKit", "ScrollEffectOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "ScrollEffectOptions",
        )),
        ("@kit.UIDesignKit", "ScrollEffectType") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "ScrollEffectType",
        )),
        ("@kit.UIDesignKit", "SelectStyle") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SelectStyle"))
        }
        ("@kit.UIDesignKit", "SnackBarIconOptions") => Some(KitEntry::Named(
            "@hms:hds.HdsSnackBar",
            "SnackBarIconOptions",
        )),
        ("@kit.UIDesignKit", "SnackBarIconType") => {
            Some(KitEntry::Named("@hms:hds.HdsSnackBar", "SnackBarIconType"))
        }
        ("@kit.UIDesignKit", "SnackBarMessageOptions") => Some(KitEntry::Named(
            "@hms:hds.HdsSnackBar",
            "SnackBarMessageOptions",
        )),
        ("@kit.UIDesignKit", "SnackBarOperationOptions") => Some(KitEntry::Named(
            "@hms:hds.HdsSnackBar",
            "SnackBarOperationOptions",
        )),
        ("@kit.UIDesignKit", "SnackBarOperationType") => Some(KitEntry::Named(
            "@hms:hds.HdsSnackBar",
            "SnackBarOperationType",
        )),
        ("@kit.UIDesignKit", "SnackBarStyleOptions") => Some(KitEntry::Named(
            "@hms:hds.HdsSnackBar",
            "SnackBarStyleOptions",
        )),
        ("@kit.UIDesignKit", "SuffixArrow") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixArrow"))
        }
        ("@kit.UIDesignKit", "SuffixArrowIconOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixArrowIconOptions",
        )),
        ("@kit.UIDesignKit", "SuffixArrowIconText") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixArrowIconText",
        )),
        ("@kit.UIDesignKit", "SuffixArrowIconTextOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixArrowIconTextOptions",
        )),
        ("@kit.UIDesignKit", "SuffixBadge") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixBadge"))
        }
        ("@kit.UIDesignKit", "SuffixBadgeAndArrow") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixBadgeAndArrow",
        )),
        ("@kit.UIDesignKit", "SuffixButton") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixButton"))
        }
        ("@kit.UIDesignKit", "SuffixCheckbox") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixCheckbox",
        )),
        ("@kit.UIDesignKit", "SuffixCustomBuilder") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixCustomBuilder",
        )),
        ("@kit.UIDesignKit", "SuffixIcon") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixIcon"))
        }
        ("@kit.UIDesignKit", "SuffixIconOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixIconOptions",
        )),
        ("@kit.UIDesignKit", "SuffixImage") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixImage"))
        }
        ("@kit.UIDesignKit", "SuffixLoadingProgress") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixLoadingProgress",
        )),
        ("@kit.UIDesignKit", "SuffixRadio") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixRadio"))
        }
        ("@kit.UIDesignKit", "SuffixSelect") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixSelect"))
        }
        ("@kit.UIDesignKit", "SuffixSubIcon") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixSubIcon",
        )),
        ("@kit.UIDesignKit", "SuffixSwitch") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixSwitch"))
        }
        ("@kit.UIDesignKit", "SuffixText") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SuffixText"))
        }
        ("@kit.UIDesignKit", "SuffixTextAndArrow") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixTextAndArrow",
        )),
        ("@kit.UIDesignKit", "SuffixToggleButton") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SuffixToggleButton",
        )),
        ("@kit.UIDesignKit", "SwipeIconConfigurations") => Some(KitEntry::Named(
            "@hms:hds.HdsStyle",
            "SwipeIconConfigurations",
        )),
        ("@kit.UIDesignKit", "SwipeIconType") => {
            Some(KitEntry::Named("@hms:hds.HdsStyle", "SwipeIconType"))
        }
        ("@kit.UIDesignKit", "SymbolGlyphOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "SymbolGlyphOptions",
        )),
        ("@kit.UIDesignKit", "SymbolType") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "SymbolType"))
        }
        ("@kit.UIDesignKit", "TextOptions") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "TextOptions"))
        }
        ("@kit.UIDesignKit", "TextStyleMode") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "TextStyleMode",
        )),
        ("@kit.UIDesignKit", "TextSymbolGlyphOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "TextSymbolGlyphOptions",
        )),
        ("@kit.UIDesignKit", "TitleBarContentOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "TitleBarContentOptions",
        )),
        ("@kit.UIDesignKit", "TitleBarStyleOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "TitleBarStyleOptions",
        )),
        ("@kit.UIDesignKit", "ToggleButtonOptions") => Some(KitEntry::Named(
            "@hms:hds.hdsBaseComponent",
            "ToggleButtonOptions",
        )),
        ("@kit.UIDesignKit", "bleedIconStyle") => {
            Some(KitEntry::Named("@hms:hds.HdsStyle", "bleedIconStyle"))
        }
        ("@kit.UIDesignKit", "hdsDrawable") => Some(KitEntry::Default("@hms:hds.hdsDrawable")),
        ("@kit.UIDesignKit", "hdsEffect") => {
            Some(KitEntry::Named("@hms:hds.hdsBaseComponent", "hdsEffect"))
        }
        ("@kit.UIDesignKit", "symbolRegister") => {
            Some(KitEntry::Default("@hms:hds.symbolRegister"))
        }

        // ── @kit.VisionKit (HMS) ───────────────────────────────────────
        ("@kit.VisionKit", "BankCardConfig") => {
            Some(KitEntry::Named("@hms:ai.CardRecognition", "BankCardConfig"))
        }
        ("@kit.VisionKit", "CallbackParam") => {
            Some(KitEntry::Named("@hms:ai.CardRecognition", "CallbackParam"))
        }
        ("@kit.VisionKit", "CardContentConfig") => Some(KitEntry::Named(
            "@hms:ai.CardRecognition",
            "CardContentConfig",
        )),
        ("@kit.VisionKit", "CardRecognition") => Some(KitEntry::Named(
            "@hms:ai.CardRecognition",
            "CardRecognition",
        )),
        ("@kit.VisionKit", "CardRecognitionConfig") => Some(KitEntry::Named(
            "@hms:ai.CardRecognition",
            "CardRecognitionConfig",
        )),
        ("@kit.VisionKit", "CardRecognitionResult") => Some(KitEntry::Named(
            "@hms:ai.CardRecognition",
            "CardRecognitionResult",
        )),
        ("@kit.VisionKit", "CardSide") => {
            Some(KitEntry::Named("@hms:ai.CardRecognition", "CardSide"))
        }
        ("@kit.VisionKit", "CardType") => {
            Some(KitEntry::Named("@hms:ai.CardRecognition", "CardType"))
        }
        ("@kit.VisionKit", "DocType") => {
            Some(KitEntry::Named("@hms:ai.DocumentScanner", "DocType"))
        }
        ("@kit.VisionKit", "DocumentScanner") => Some(KitEntry::Named(
            "@hms:ai.DocumentScanner",
            "DocumentScanner",
        )),
        ("@kit.VisionKit", "DocumentScannerConfig") => Some(KitEntry::Named(
            "@hms:ai.DocumentScanner",
            "DocumentScannerConfig",
        )),
        ("@kit.VisionKit", "DocumentScannerResultCallback") => Some(KitEntry::Named(
            "@hms:ai.DocumentScanner",
            "DocumentScannerResultCallback",
        )),
        ("@kit.VisionKit", "EditTab") => {
            Some(KitEntry::Named("@hms:ai.DocumentScanner", "EditTab"))
        }
        ("@kit.VisionKit", "FilterId") => {
            Some(KitEntry::Named("@hms:ai.DocumentScanner", "FilterId"))
        }
        ("@kit.VisionKit", "IdCardConfig") => {
            Some(KitEntry::Named("@hms:ai.CardRecognition", "IdCardConfig"))
        }
        ("@kit.VisionKit", "SaveOption") => {
            Some(KitEntry::Named("@hms:ai.DocumentScanner", "SaveOption"))
        }
        ("@kit.VisionKit", "ShootingMode") => {
            Some(KitEntry::Named("@hms:ai.DocumentScanner", "ShootingMode"))
        }
        ("@kit.VisionKit", "interactiveLiveness") => {
            Some(KitEntry::Default("@hms:ai.interactiveLiveness"))
        }
        ("@kit.VisionKit", "visionImageAnalyzer") => {
            Some(KitEntry::Default("@hms:ai.visionImageAnalyzer"))
        }

        // ── @kit.WalletKit (HMS) ───────────────────────────────────────
        ("@kit.WalletKit", "walletPass") => Some(KitEntry::Default("@hms:core.payment.walletPass")),
        ("@kit.WalletKit", "walletTransitCard") => {
            Some(KitEntry::Default("@hms:core.payment.walletTransitCard"))
        }

        // ── @kit.WearEngine (HMS) ───────────────────────────────────────
        ("@kit.WearEngine", "wearEngine") => Some(KitEntry::Default("@hms:health.wearEngine")),

        // ── @kit.WeatherServiceKit (HMS) ───────────────────────────────────────
        ("@kit.WeatherServiceKit", "weatherService") => {
            Some(KitEntry::Default("@hms:core.weather"))
        }

        _ => None,
    }
}

/// Returns `true` if `spec` is a recognised `@kit.XxxKit` specifier.
pub fn is_kit_specifier(spec: &str) -> bool {
    ModuleSpecifierText::new(spec).is_kit()
}
