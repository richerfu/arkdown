/// Maps an ArkUI state decorator name to the ViewPU backing class name.
///
/// These backing classes are part of the ArkUI state management runtime and
/// are injected by the OHOS SDK. The bundler only needs to emit the correct
/// class names — the runtime provides the implementations.
pub fn state_decorator_class(decorator: &str) -> &'static str {
    match decorator {
        "State" => "ObservedPropertySimplePU",
        "Prop" => "SynchedPropertySimpleOneWayPU",
        "Link" => "SynchedPropertySimpleTwoWayPU",
        "StorageLink" => "AppStorageLinkedProperty",
        "StorageProp" => "AppStorageProperty",
        "LocalStorageLink" => "LocalStorageLinkedProperty",
        "LocalStorageProp" => "LocalStorageProperty",
        "Provide" | "Consume" => "ObservedPropertyObjectPU",
        "ObjectLink" => "SynchedPropertyNesedObjectPU",
        "BuilderParam" => "WrappedBuilder",
        _ => "ObservedPropertySimplePU",
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
            | "BuilderParam"
    )
}

/// Returns true if the given state decorator contributes a slot to `updateStateVars`.
pub fn has_update_state_vars_slot(decorator: &str) -> bool {
    matches!(decorator, "Prop" | "LocalStorageProp" | "BuilderParam")
}
