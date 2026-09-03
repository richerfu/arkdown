use oxc_ast::ast::{Decorator, Expression};

pub(crate) struct ArkuiDecoratorName;

impl ArkuiDecoratorName {
    pub(crate) const ANIMATABLE_EXTEND: &'static str = "AnimatableExtend";
    pub(crate) const BUILDER: &'static str = "Builder";
    pub(crate) const BUILDER_PARAM: &'static str = "BuilderParam";
    pub(crate) const COMPONENT: &'static str = "Component";
    pub(crate) const COMPONENT_V2: &'static str = "ComponentV2";
    pub(crate) const CONCURRENT: &'static str = "Concurrent";
    pub(crate) const CONSUME: &'static str = "Consume";
    pub(crate) const CONSUMER: &'static str = "Consumer";
    pub(crate) const CUSTOM_DIALOG: &'static str = "CustomDialog";
    pub(crate) const CUSTOM_ENV: &'static str = "CustomEnv";
    pub(crate) const ENTRY: &'static str = "Entry";
    pub(crate) const EXTEND: &'static str = "Extend";
    pub(crate) const LOCAL_BUILDER: &'static str = "LocalBuilder";
    pub(crate) const OBSERVED: &'static str = "Observed";
    pub(crate) const OBSERVED_V2: &'static str = "ObservedV2";
    pub(crate) const PROVIDE: &'static str = "Provide";
    pub(crate) const PROVIDER: &'static str = "Provider";
    pub(crate) const REUSABLE: &'static str = "Reusable";
    pub(crate) const REUSABLE_V2: &'static str = "ReusableV2";
    pub(crate) const SENDABLE: &'static str = "Sendable";
    pub(crate) const STYLES: &'static str = "Styles";
    pub(crate) const PREVIEW: &'static str = "Preview";
    pub(crate) const TRACK: &'static str = "Track";
    pub(crate) const TYPE: &'static str = "Type";
    pub(crate) const WATCH: &'static str = "Watch";

    pub(crate) const STATE: &'static str = "State";
    pub(crate) const STORAGE_LINK: &'static str = "StorageLink";
    pub(crate) const STORAGE_PROP: &'static str = "StorageProp";
    pub(crate) const STORAGE_PROP_REF: &'static str = "StoragePropRef";
    pub(crate) const LOCAL_STORAGE_LINK: &'static str = "LocalStorageLink";
    pub(crate) const LOCAL_STORAGE_PROP: &'static str = "LocalStorageProp";
    pub(crate) const LOCAL_STORAGE_PROP_REF: &'static str = "LocalStoragePropRef";
    pub(crate) const LINK: &'static str = "Link";
    pub(crate) const OBJECT_LINK: &'static str = "ObjectLink";
    pub(crate) const PROP: &'static str = "Prop";
    pub(crate) const PROP_REF: &'static str = "PropRef";
    pub(crate) const LOCAL: &'static str = "Local";
    pub(crate) const ONCE: &'static str = "Once";
    pub(crate) const PARAM: &'static str = "Param";
    pub(crate) const EVENT: &'static str = "Event";
    pub(crate) const ENV: &'static str = "Env";
    pub(crate) const TRACE: &'static str = "Trace";
    pub(crate) const REQUIRE: &'static str = "Require";
    pub(crate) const COMPUTED: &'static str = "Computed";
    pub(crate) const MONITOR: &'static str = "Monitor";
    pub(crate) const SYNC_MONITOR: &'static str = "SyncMonitor";
    pub(crate) const COMPONENT_INIT: &'static str = "ComponentInit";
    pub(crate) const COMPONENT_APPEAR: &'static str = "ComponentAppear";
    pub(crate) const COMPONENT_BUILT: &'static str = "ComponentBuilt";
    pub(crate) const COMPONENT_ATTACH: &'static str = "ComponentAttach";
    pub(crate) const COMPONENT_DETACH: &'static str = "ComponentDetach";
    pub(crate) const COMPONENT_REUSE: &'static str = "ComponentReuse";
    pub(crate) const COMPONENT_RECYCLE: &'static str = "ComponentRecycle";
    pub(crate) const COMPONENT_DISAPPEAR: &'static str = "ComponentDisappear";
    pub(crate) const COMPONENT_ACTIVE: &'static str = "ComponentActive";
    pub(crate) const COMPONENT_INACTIVE: &'static str = "ComponentInactive";

    pub(crate) fn is_top_level_function(name: &str) -> bool {
        matches!(
            name,
            Self::BUILDER
                | Self::LOCAL_BUILDER
                | Self::STYLES
                | Self::EXTEND
                | Self::ANIMATABLE_EXTEND
                | Self::CONCURRENT
        )
    }

    pub(crate) fn appends_parent_parameter(name: &str) -> bool {
        matches!(name, Self::BUILDER | Self::LOCAL_BUILDER)
    }

    pub(crate) fn is_extend_function(name: &str) -> bool {
        matches!(name, Self::EXTEND | Self::ANIMATABLE_EXTEND)
    }

    pub(crate) fn is_component_struct(name: &str) -> bool {
        matches!(name, Self::COMPONENT | Self::CUSTOM_DIALOG)
    }

    pub(crate) fn is_component_struct_or_dialog(name: &str) -> bool {
        matches!(
            name,
            Self::COMPONENT | Self::COMPONENT_V2 | Self::CUSTOM_DIALOG
        )
    }

    pub(crate) fn is_provider_alias(name: &str) -> bool {
        matches!(
            name,
            Self::PROVIDE | Self::CONSUME | Self::PROVIDER | Self::CONSUMER
        )
    }

    pub(crate) fn is_custom_component_update_prop(name: &str) -> bool {
        matches!(name, Self::PROP | Self::OBJECT_LINK)
    }

    pub(crate) fn contributes_update_state_vars(name: &str) -> bool {
        matches!(
            name,
            Self::PROP | Self::OBJECT_LINK | Self::PARAM | Self::ONCE | Self::PROP_REF
        )
    }

    pub(crate) fn has_initially_provided_value_slot(name: &str) -> bool {
        matches!(
            name,
            Self::STATE
                | Self::PROVIDE
                | Self::OBJECT_LINK
                | Self::BUILDER_PARAM
                | Self::LOCAL
                | Self::PROVIDER
        )
    }

    pub(crate) fn primary_state(names: &[&str]) -> Option<&'static str> {
        STATE_DECORATOR_PRIORITY.iter().copied().find(|candidate| {
            names.iter().any(|name| name == candidate)
                && ArkuiStateDecorator::new(candidate).is_state()
        })
    }
}

const STATE_DECORATOR_PRIORITY: &[&str] = &[
    ArkuiDecoratorName::STATE,
    ArkuiDecoratorName::STORAGE_LINK,
    ArkuiDecoratorName::STORAGE_PROP,
    ArkuiDecoratorName::LOCAL_STORAGE_LINK,
    ArkuiDecoratorName::LOCAL_STORAGE_PROP,
    ArkuiDecoratorName::LINK,
    ArkuiDecoratorName::OBJECT_LINK,
    ArkuiDecoratorName::PROP,
    ArkuiDecoratorName::LOCAL_STORAGE_PROP_REF,
    ArkuiDecoratorName::STORAGE_PROP_REF,
    ArkuiDecoratorName::PROP_REF,
    ArkuiDecoratorName::PROVIDE,
    ArkuiDecoratorName::CONSUME,
    ArkuiDecoratorName::BUILDER_PARAM,
    ArkuiDecoratorName::LOCAL,
    ArkuiDecoratorName::ONCE,
    ArkuiDecoratorName::PARAM,
    ArkuiDecoratorName::PROVIDER,
    ArkuiDecoratorName::CONSUMER,
    ArkuiDecoratorName::EVENT,
    ArkuiDecoratorName::ENV,
    ArkuiDecoratorName::CUSTOM_ENV,
    ArkuiDecoratorName::TRACE,
];

pub(crate) struct ArkuiDecorator<'a> {
    decorator: &'a Decorator<'a>,
}

impl<'a> ArkuiDecorator<'a> {
    pub(crate) fn new(decorator: &'a Decorator<'a>) -> Self {
        Self { decorator }
    }

    pub(crate) fn name(&self) -> &'a str {
        match &self.decorator.expression {
            Expression::Identifier(identifier) => identifier.name.as_str(),
            Expression::CallExpression(call) => match &call.callee {
                Expression::Identifier(identifier) => identifier.name.as_str(),
                _ => "",
            },
            _ => "",
        }
    }

    /// Extract the first string-literal argument from a decorator call expression.
    ///
    /// Used to extract the handler name from `@Watch("handler")` and the alias
    /// from `@Provide("alias")` / `@Consume("alias")`.  Returns `None` if the
    /// decorator has no arguments or the first argument is not a string literal.
    pub(super) fn string_arg(&self) -> Option<String> {
        if let Expression::CallExpression(call) = &self.decorator.expression {
            if let Some(arg) = call.arguments.first() {
                if let Some(Expression::StringLiteral(lit)) = arg.as_expression() {
                    return Some(lit.value.to_string());
                }
            }
        }
        None
    }

    pub(crate) fn arg_expression(&self) -> Option<&'a Expression<'a>> {
        let Expression::CallExpression(call) = &self.decorator.expression else {
            return None;
        };
        call.arguments.first()?.as_expression()
    }

    pub(super) fn identifier_arg(&self) -> Option<String> {
        match self.arg_expression()? {
            Expression::Identifier(identifier) => Some(identifier.name.to_string()),
            Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
            _ => None,
        }
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
    Local,
    /// `new BackingClass(params.name, this, "name")` — `@Prop`, `@Link`, `@ObjectLink`.
    /// The initial value is supplied by the parent via the params object.
    Param,
    /// `this.initializeConsume("alias", "name")` — `@Consume`.
    /// The property is resolved from a parent `@Provide` by key at runtime.
    Consume,
    /// `this.name = init ?? undefined` — `@BuilderParam`.
    /// Assigned directly onto `this` (no `__` prefix) from field init or undefined.
    BuilderParam,
    /// Storage-backed properties (`@StorageProp`, `@StorageLink`,
    /// `@LocalStorageProp`, `@LocalStorageLink`) use runtime storage APIs.
    Storage,
}

pub(crate) struct ArkuiStateDecorator<'a> {
    name: &'a str,
}

impl<'a> ArkuiStateDecorator<'a> {
    pub(crate) fn new(name: &'a str) -> Self {
        Self { name }
    }

    pub(crate) fn class_for(&self, is_simple_type: bool) -> &'static str {
        match self.name {
            ArkuiDecoratorName::STATE
            | ArkuiDecoratorName::PROVIDE
            | ArkuiDecoratorName::LOCAL
            | ArkuiDecoratorName::PROVIDER
            | ArkuiDecoratorName::ENV
            | ArkuiDecoratorName::TRACE => {
                if is_simple_type {
                    "ObservedPropertySimplePU"
                } else {
                    "ObservedPropertyObjectPU"
                }
            }
            ArkuiDecoratorName::PROP
            | ArkuiDecoratorName::PARAM
            | ArkuiDecoratorName::ONCE
            | ArkuiDecoratorName::PROP_REF => {
                if is_simple_type {
                    "SynchedPropertySimpleOneWayPU"
                } else {
                    "SynchedPropertyObjectOneWayPU"
                }
            }
            ArkuiDecoratorName::LINK => {
                if is_simple_type {
                    "SynchedPropertySimpleTwoWayPU"
                } else {
                    "SynchedPropertyObjectTwoWayPU"
                }
            }
            ArkuiDecoratorName::OBJECT_LINK => "SynchedPropertyNesedObjectPU",
            // Consume and the storage decorators all share ObservedPropertyAbstractPU as
            // the declared field type in PU mode (they are resolved/linked at runtime).
            ArkuiDecoratorName::CONSUME
            | ArkuiDecoratorName::CONSUMER
            | ArkuiDecoratorName::STORAGE_LINK
            | ArkuiDecoratorName::STORAGE_PROP
            | ArkuiDecoratorName::STORAGE_PROP_REF
            | ArkuiDecoratorName::LOCAL_STORAGE_LINK
            | ArkuiDecoratorName::LOCAL_STORAGE_PROP
            | ArkuiDecoratorName::LOCAL_STORAGE_PROP_REF => "ObservedPropertyAbstractPU",
            ArkuiDecoratorName::BUILDER_PARAM => "WrappedBuilder",
            _ => "ObservedPropertySimplePU",
        }
    }

    pub(crate) fn declaration_class_for(&self, is_simple_type: bool) -> &'static str {
        match self.name {
            ArkuiDecoratorName::PROP
            | ArkuiDecoratorName::PARAM
            | ArkuiDecoratorName::ONCE
            | ArkuiDecoratorName::PROP_REF
                if !is_simple_type =>
            {
                "SynchedPropertySimpleOneWayPU"
            }
            ArkuiDecoratorName::LINK if !is_simple_type => "SynchedPropertySimpleOneWayPU",
            _ => self.class_for(is_simple_type),
        }
    }

    /// Returns the constructor initialisation strategy for a given decorator.
    ///
    /// Derived from `updateObservedPropertyPU`, `updateSynchedPropertyTwoWayPU`,
    /// `updateSynchedPropertyOneWayPU`, `updateSynchedPropertyNesedObjectPU`,
    /// `updateConsumeProperty`, and `updateBuilderParamProperty` in ets-loader
    /// `process_component_member.js`.
    pub(crate) fn constructor_init_kind(&self) -> ConstructorInitKind {
        match self.name {
            ArkuiDecoratorName::STATE
            | ArkuiDecoratorName::PROVIDE
            | ArkuiDecoratorName::LOCAL
            | ArkuiDecoratorName::PROVIDER
            | ArkuiDecoratorName::ENV
            | ArkuiDecoratorName::TRACE => ConstructorInitKind::Local,
            ArkuiDecoratorName::PROP
            | ArkuiDecoratorName::PARAM
            | ArkuiDecoratorName::ONCE
            | ArkuiDecoratorName::LINK
            | ArkuiDecoratorName::OBJECT_LINK
            | ArkuiDecoratorName::PROP_REF => ConstructorInitKind::Param,
            ArkuiDecoratorName::CONSUME | ArkuiDecoratorName::CONSUMER => {
                ConstructorInitKind::Consume
            }
            ArkuiDecoratorName::BUILDER_PARAM => ConstructorInitKind::BuilderParam,
            ArkuiDecoratorName::STORAGE_LINK
            | ArkuiDecoratorName::STORAGE_PROP
            | ArkuiDecoratorName::STORAGE_PROP_REF
            | ArkuiDecoratorName::LOCAL_STORAGE_LINK
            | ArkuiDecoratorName::LOCAL_STORAGE_PROP
            | ArkuiDecoratorName::LOCAL_STORAGE_PROP_REF => ConstructorInitKind::Storage,
            _ => ConstructorInitKind::Local,
        }
    }

    /// Returns true if the given decorator is a state management decorator
    /// (i.e. it requires a backing field in the ViewPU class).
    pub(crate) fn is_state(&self) -> bool {
        matches!(
            self.name,
            ArkuiDecoratorName::STATE
            | ArkuiDecoratorName::PROP
            | ArkuiDecoratorName::LINK
            | ArkuiDecoratorName::STORAGE_LINK
            | ArkuiDecoratorName::STORAGE_PROP
            | ArkuiDecoratorName::LOCAL_STORAGE_LINK
            | ArkuiDecoratorName::LOCAL_STORAGE_PROP
            | ArkuiDecoratorName::PROVIDE
            | ArkuiDecoratorName::CONSUME
            | ArkuiDecoratorName::OBJECT_LINK
            | ArkuiDecoratorName::PROP_REF
            | ArkuiDecoratorName::BUILDER_PARAM
            | ArkuiDecoratorName::LOCAL
            | ArkuiDecoratorName::PARAM
            | ArkuiDecoratorName::ONCE
            | ArkuiDecoratorName::PROVIDER
            | ArkuiDecoratorName::CONSUMER
            | ArkuiDecoratorName::ENV
            | ArkuiDecoratorName::TRACE
            // @Event: treated as regular field (like @BuilderParam without backing store)
            // @Require: meta-decorator marking required params (no backing store)
            // (Neither @Event nor @Require are in is_state_decorator — they fall through
            //  to regular_fields which generates direct assignment without __ prefix)
            | ArkuiDecoratorName::STORAGE_PROP_REF
            | ArkuiDecoratorName::LOCAL_STORAGE_PROP_REF
        )
    }

    /// Returns true if the given state decorator contributes a slot to `updateStateVars`.
    ///
    /// Only decorators that receive new values from the parent on re-render need a slot.
    /// Derived from the `setUpdateParamsDecorators` set in ets-loader and the SPEC-004a table.
    pub(crate) fn has_update_state_vars_slot(&self) -> bool {
        ArkuiDecoratorName::contributes_update_state_vars(self.name)
    }

    pub(crate) fn has_initially_provided_value_slot(&self) -> bool {
        ArkuiDecoratorName::has_initially_provided_value_slot(self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_decorator_class_mappings() {
        assert_eq!(
            ArkuiStateDecorator::new("State").class_for(true),
            "ObservedPropertySimplePU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("Provide").class_for(true),
            "ObservedPropertySimplePU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("Prop").class_for(true),
            "SynchedPropertySimpleOneWayPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("Link").class_for(true),
            "SynchedPropertySimpleTwoWayPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("ObjectLink").class_for(true),
            "SynchedPropertyNesedObjectPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("Consume").class_for(true),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("StorageLink").class_for(true),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("StorageProp").class_for(true),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("LocalStorageLink").class_for(true),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("LocalStorageProp").class_for(true),
            "ObservedPropertyAbstractPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("BuilderParam").class_for(true),
            "WrappedBuilder"
        );
        assert_eq!(
            ArkuiStateDecorator::new("Local").class_for(true),
            "ObservedPropertySimplePU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("Param").class_for(true),
            "SynchedPropertySimpleOneWayPU"
        );
        assert_eq!(
            ArkuiStateDecorator::new("Consumer").class_for(true),
            "ObservedPropertyAbstractPU"
        );
    }

    #[test]
    fn test_constructor_init_kind() {
        assert_eq!(
            ArkuiStateDecorator::new("State").constructor_init_kind(),
            ConstructorInitKind::Local
        );
        assert_eq!(
            ArkuiStateDecorator::new("Provide").constructor_init_kind(),
            ConstructorInitKind::Local
        );
        assert_eq!(
            ArkuiStateDecorator::new("Prop").constructor_init_kind(),
            ConstructorInitKind::Param
        );
        assert_eq!(
            ArkuiStateDecorator::new("Link").constructor_init_kind(),
            ConstructorInitKind::Param
        );
        assert_eq!(
            ArkuiStateDecorator::new("ObjectLink").constructor_init_kind(),
            ConstructorInitKind::Param
        );
        assert_eq!(
            ArkuiStateDecorator::new("Consume").constructor_init_kind(),
            ConstructorInitKind::Consume
        );
        assert_eq!(
            ArkuiStateDecorator::new("BuilderParam").constructor_init_kind(),
            ConstructorInitKind::BuilderParam
        );
        assert_eq!(
            ArkuiStateDecorator::new("StorageProp").constructor_init_kind(),
            ConstructorInitKind::Storage
        );
        assert_eq!(
            ArkuiStateDecorator::new("LocalStorageLink").constructor_init_kind(),
            ConstructorInitKind::Storage
        );
        assert_eq!(
            ArkuiStateDecorator::new("Local").constructor_init_kind(),
            ConstructorInitKind::Local
        );
        assert_eq!(
            ArkuiStateDecorator::new("Param").constructor_init_kind(),
            ConstructorInitKind::Param
        );
        assert_eq!(
            ArkuiStateDecorator::new("Consumer").constructor_init_kind(),
            ConstructorInitKind::Consume
        );
    }

    #[test]
    fn test_has_update_state_vars_slot() {
        assert!(ArkuiStateDecorator::new("Prop").has_update_state_vars_slot());
        assert!(ArkuiStateDecorator::new("Param").has_update_state_vars_slot());
        assert!(!ArkuiStateDecorator::new("BuilderParam").has_update_state_vars_slot());
        assert!(!ArkuiStateDecorator::new("LocalStorageProp").has_update_state_vars_slot());
        assert!(!ArkuiStateDecorator::new("State").has_update_state_vars_slot());
        assert!(!ArkuiStateDecorator::new("Link").has_update_state_vars_slot());
        assert!(!ArkuiStateDecorator::new("Provide").has_update_state_vars_slot());
        assert!(!ArkuiStateDecorator::new("Consume").has_update_state_vars_slot());
        assert!(ArkuiStateDecorator::new("ObjectLink").has_update_state_vars_slot());
    }
}
