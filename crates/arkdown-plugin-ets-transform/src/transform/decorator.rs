use oxc_ast::ast::{Decorator, Expression};

pub(super) fn decorator_name<'a>(decorator: &'a Decorator<'a>) -> &'a str {
    match &decorator.expression {
        Expression::Identifier(identifier) => identifier.name.as_str(),
        Expression::CallExpression(call) => match &call.callee {
            Expression::Identifier(identifier) => identifier.name.as_str(),
            _ => "",
        },
        _ => "",
    }
}

/// Describes how a state-managed field is initialised in the ViewPU constructor.
///
/// The patterns are derived from the hvigor ets-loader `process_component_member.js`
/// functions `updateObservedPropertyPU`, `updateSynchedPropertyTwoWayPU`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructorInitKind {
    /// `new BackingClass(fieldInit, this, "name")` — `@State`, `@Provide`.
    /// The initial value comes from the field's own initializer expression.
    NewWithLocalInit,
    /// `new BackingClass(params.name, this, "name")` — `@Prop`, `@Link`, `@ObjectLink`.
    /// The initial value is supplied by the parent via the params object.
    NewWithParamInit,
    /// `this.initializeConsume("alias", "name")` — `@Consume`.
    /// The property is resolved from a parent `@Provide` by key at runtime.
    ConsumeInit,
    /// `this.name = init ?? undefined` — `@BuilderParam`.
    /// Assigned directly onto `this` (no `__` prefix) from field init or undefined.
    BuilderParamInit,
    /// Storage-backed properties (`@StorageProp`, `@StorageLink`,
    /// `@LocalStorageProp`, `@LocalStorageLink`) use runtime storage APIs.
    StorageInit,
}

/// Maps an ArkUI state decorator name to the ViewPU backing class name.
///
/// These backing classes are part of the ArkUI state management runtime and
/// are injected by the OHOS SDK.  The bundler only needs to emit the correct
/// class names — the runtime provides the implementations.
///
/// Derived from `createTypeReferencePU` in ets-loader `process_component_class.js`
/// (PU / partial-update mode, API >= 9).
pub fn state_decorator_class(decorator: &str) -> &'static str {
    state_decorator_class_for(decorator, true)
}

pub fn state_decorator_class_for(decorator: &str, is_simple_type: bool) -> &'static str {
    match decorator {
        "State" | "Provide" | "Local" | "Provider" | "Env" | "Trace" => {
            if is_simple_type {
                "ObservedPropertySimplePU"
            } else {
                "ObservedPropertyObjectPU"
            }
        }
        "Prop" | "Param" | "Once" | "PropRef" => {
            if is_simple_type {
                "SynchedPropertySimpleOneWayPU"
            } else {
                "SynchedPropertyObjectOneWayPU"
            }
        }
        "Link" => {
            if is_simple_type {
                "SynchedPropertySimpleTwoWayPU"
            } else {
                "SynchedPropertyObjectTwoWayPU"
            }
        }
        "ObjectLink" => "SynchedPropertyNesedObjectPU",
        // Consume and the storage decorators all share ObservedPropertyAbstractPU as
        // the declared field type in PU mode (they are resolved/linked at runtime).
        "Consume"
        | "Consumer"
        | "StorageLink"
        | "StorageProp"
        | "StoragePropRef"
        | "LocalStorageLink"
        | "LocalStorageProp"
        | "LocalStoragePropRef" => "ObservedPropertyAbstractPU",
        "BuilderParam" => "WrappedBuilder",
        _ => "ObservedPropertySimplePU",
    }
}

pub fn state_decorator_decl_class_for(decorator: &str, is_simple_type: bool) -> &'static str {
    match decorator {
        "Prop" | "Param" | "Once" | "PropRef" if !is_simple_type => "SynchedPropertySimpleOneWayPU",
        "Link" if !is_simple_type => "SynchedPropertySimpleOneWayPU",
        _ => state_decorator_class_for(decorator, is_simple_type),
    }
}

/// Returns the constructor initialisation strategy for a given decorator.
///
/// Derived from `updateObservedPropertyPU`, `updateSynchedPropertyTwoWayPU`,
/// `updateSynchedPropertyOneWayPU`, `updateSynchedPropertyNesedObjectPU`,
/// `updateConsumeProperty`, and `updateBuilderParamProperty` in ets-loader
/// `process_component_member.js`.
pub fn constructor_init_kind(decorator: &str) -> ConstructorInitKind {
    match decorator {
        "State" | "Provide" | "Local" | "Provider" | "Env" | "Trace" => {
            ConstructorInitKind::NewWithLocalInit
        }
        "Prop" | "Param" | "Once" | "Link" | "ObjectLink" | "PropRef" => {
            ConstructorInitKind::NewWithParamInit
        }
        "Consume" | "Consumer" => ConstructorInitKind::ConsumeInit,
        "BuilderParam" => ConstructorInitKind::BuilderParamInit,
        "StorageLink"
        | "StorageProp"
        | "StoragePropRef"
        | "LocalStorageLink"
        | "LocalStorageProp"
        | "LocalStoragePropRef" => ConstructorInitKind::StorageInit,
        _ => ConstructorInitKind::NewWithLocalInit,
    }
}

/// Returns true if the given decorator is a state management decorator
/// (i.e. it requires a backing field in the ViewPU class).
pub fn is_state_decorator(decorator: &str) -> bool {
    matches!(
        decorator,
        "State"
            | "Prop"
            | "Link"
            | "StorageLink"
            | "StorageProp"
            | "LocalStorageLink"
            | "LocalStorageProp"
            | "Provide"
            | "Consume"
            | "ObjectLink"
            | "PropRef"
            | "BuilderParam"
            | "Local"
            | "Param"
            | "Once"
            | "Provider"
            | "Consumer"
            | "Env"
            | "Trace"
            | "StoragePropRef"
            | "LocalStoragePropRef"
    )
}

/// Returns true if the given state decorator contributes a slot to `updateStateVars`.
///
/// Only decorators that receive new values from the parent on re-render need a slot.
/// Derived from the `setUpdateParamsDecorators` set in ets-loader and the SPEC-004a table.
pub fn has_update_state_vars_slot(decorator: &str) -> bool {
    matches!(decorator, "Prop" | "Param" | "Once" | "PropRef")
}

pub fn has_initially_provided_value_slot(decorator: &str) -> bool {
    matches!(
        decorator,
        "State" | "Provide" | "ObjectLink" | "BuilderParam" | "Local" | "Provider"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_decorator_class_mappings() {
        assert_eq!(state_decorator_class("State"), "ObservedPropertySimplePU");
        assert_eq!(state_decorator_class("Provide"), "ObservedPropertySimplePU");
        assert_eq!(
            state_decorator_class("Prop"),
            "SynchedPropertySimpleOneWayPU"
        );
        assert_eq!(
            state_decorator_class("Link"),
            "SynchedPropertySimpleTwoWayPU"
        );
        assert_eq!(
            state_decorator_class("ObjectLink"),
            "SynchedPropertyNesedObjectPU"
        );
        assert_eq!(
            state_decorator_class("Consume"),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            state_decorator_class("StorageLink"),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            state_decorator_class("StorageProp"),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            state_decorator_class("LocalStorageLink"),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            state_decorator_class("LocalStorageProp"),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(state_decorator_class("BuilderParam"), "WrappedBuilder");
        assert_eq!(state_decorator_class("Local"), "ObservedPropertySimplePU");
        assert_eq!(
            state_decorator_class("Param"),
            "SynchedPropertySimpleOneWayPU"
        );
        assert_eq!(
            state_decorator_class("Consumer"),
            "ObservedPropertyAbstractPU"
        );
    }

    #[test]
    fn test_constructor_init_kind() {
        assert_eq!(
            constructor_init_kind("State"),
            ConstructorInitKind::NewWithLocalInit
        );
        assert_eq!(
            constructor_init_kind("Provide"),
            ConstructorInitKind::NewWithLocalInit
        );
        assert_eq!(
            constructor_init_kind("Prop"),
            ConstructorInitKind::NewWithParamInit
        );
        assert_eq!(
            constructor_init_kind("Link"),
            ConstructorInitKind::NewWithParamInit
        );
        assert_eq!(
            constructor_init_kind("ObjectLink"),
            ConstructorInitKind::NewWithParamInit
        );
        assert_eq!(
            constructor_init_kind("Consume"),
            ConstructorInitKind::ConsumeInit
        );
        assert_eq!(
            constructor_init_kind("BuilderParam"),
            ConstructorInitKind::BuilderParamInit
        );
        assert_eq!(
            constructor_init_kind("StorageProp"),
            ConstructorInitKind::StorageInit
        );
        assert_eq!(
            constructor_init_kind("LocalStorageLink"),
            ConstructorInitKind::StorageInit
        );
        assert_eq!(
            constructor_init_kind("Local"),
            ConstructorInitKind::NewWithLocalInit
        );
        assert_eq!(
            constructor_init_kind("Param"),
            ConstructorInitKind::NewWithParamInit
        );
        assert_eq!(
            constructor_init_kind("Consumer"),
            ConstructorInitKind::ConsumeInit
        );
    }

    #[test]
    fn test_has_update_state_vars_slot() {
        assert!(has_update_state_vars_slot("Prop"));
        assert!(has_update_state_vars_slot("Param"));
        assert!(!has_update_state_vars_slot("BuilderParam"));
        assert!(!has_update_state_vars_slot("LocalStorageProp"));
        assert!(!has_update_state_vars_slot("State"));
        assert!(!has_update_state_vars_slot("Link"));
        assert!(!has_update_state_vars_slot("Provide"));
        assert!(!has_update_state_vars_slot("Consume"));
        assert!(!has_update_state_vars_slot("ObjectLink"));
    }
}
