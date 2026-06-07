pub(crate) fn pandasm_record_name(class_name: &str) -> String {
    let mut descriptor = class_name;
    let rank = descriptor.bytes().take_while(|&byte| byte == b'[').count();
    descriptor = &descriptor[rank..];

    let component = if let Some(name) = descriptor
        .strip_prefix('L')
        .and_then(|name| name.strip_suffix(';'))
    {
        name.to_string()
    } else if rank == 0 {
        return class_name.to_string();
    } else {
        primitive_name_from_descriptor(descriptor)
            .unwrap_or(descriptor)
            .to_string()
    };

    if rank == 0 {
        component
    } else {
        format!("{}{}", component, "[]".repeat(rank))
    }
}

pub(crate) fn class_name_from_pandasm_record(record_name: &str) -> String {
    if record_name.starts_with('L') && record_name.ends_with(';') {
        return record_name.to_string();
    }
    match record_name {
        "_ESModuleRecord"
        | "_ESScopeNamesRecord"
        | "_HasTopLevelAwait"
        | "_SharedModuleRecord"
        | "_CommonJsRecord"
        | "_ESConcurrentModuleRequestsAnnotation"
        | "_ESExpectedPropertyCountAnnotation"
        | "_ESSlotNumberAnnotation" => record_name.to_string(),
        "_GLOBAL" => "L_GLOBAL;".to_string(),
        _ => format!("L{record_name};"),
    }
}

pub(crate) fn full_method_name(class_name: &str, method_name: &str) -> String {
    let record_name = pandasm_record_name(class_name);
    if is_system_record_name(&record_name) {
        format!(".{method_name}")
    } else {
        format!("{record_name}.{method_name}")
    }
}

pub(crate) fn short_method_name(record_name: &str, method_name: &str) -> String {
    let record_name = pandasm_record_name(record_name);
    if is_system_record_name(&record_name) {
        return method_name
            .strip_prefix('.')
            .unwrap_or(method_name)
            .to_string();
    }
    method_name
        .strip_prefix(record_name.as_str())
        .and_then(|name| name.strip_prefix('.'))
        .unwrap_or(method_name)
        .to_string()
}

pub(crate) fn literal_array_key(class_name: &str, literal_index: usize) -> String {
    format!("{}_{}", pandasm_record_name(class_name), literal_index)
}

pub(crate) fn literal_array_record_name(key: &str) -> Option<&str> {
    key.rsplit_once('_')
        .and_then(|(record_name, index)| index.parse::<u32>().ok().map(|_| record_name))
}

fn is_system_record_name(record_name: &str) -> bool {
    record_name == "_GLOBAL" || record_name.contains('[')
}

fn primitive_name_from_descriptor(descriptor: &str) -> Option<&'static str> {
    match descriptor {
        "Z" => Some("u1"),
        "B" => Some("i8"),
        "H" => Some("u8"),
        "S" => Some("i16"),
        "C" => Some("u16"),
        "I" => Some("i32"),
        "U" => Some("u32"),
        "F" => Some("f32"),
        "D" => Some("f64"),
        "J" => Some("i64"),
        "Q" => Some("u64"),
        "V" => Some("void"),
        "A" => Some("any"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_names_follow_pandasm_descriptor_rules() {
        assert_eq!(pandasm_record_name("Lentry/main;"), "entry/main");
        assert_eq!(pandasm_record_name("[[I"), "i32[][]");
        assert_eq!(pandasm_record_name("_ESModuleRecord"), "_ESModuleRecord");
    }

    #[test]
    fn method_names_follow_abc2program_full_name_rules() {
        assert_eq!(
            full_method_name("Lentry/main;", "func_main_0"),
            "entry/main.func_main_0"
        );
        assert_eq!(
            short_method_name("Lentry/main;", "entry/main.func_main_0"),
            "func_main_0"
        );
        assert_eq!(full_method_name("L_GLOBAL;", "func_main_0"), ".func_main_0");
        assert_eq!(
            short_method_name("L_GLOBAL;", ".func_main_0"),
            "func_main_0"
        );
    }
}
