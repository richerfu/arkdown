const VIEWPU_FINALIZE_CONSTRUCTION_SHIM: &str = concat!(
    "if (!(\"finalizeConstruction\" in ViewPU.prototype)) {\n",
    "    Reflect.set(ViewPU.prototype, \"finalizeConstruction\", () => { });\n",
    "}\n",
);

pub(super) fn finalize_viewpu_output(source: &str) -> String {
    let out = strip_leading_file_comments(source);
    let out = ensure_viewpu_finalize_construction_shim(&out);
    hoist_viewpu_params_interfaces(&out)
}

fn ensure_viewpu_finalize_construction_shim(source: &str) -> String {
    if !source.contains("extends ViewPU")
        || source.contains("\"finalizeConstruction\" in ViewPU.prototype")
    {
        return source.to_string();
    }
    let mut out = String::with_capacity(VIEWPU_FINALIZE_CONSTRUCTION_SHIM.len() + source.len());
    out.push_str(VIEWPU_FINALIZE_CONSTRUCTION_SHIM);
    out.push_str(source);
    out
}

fn strip_leading_file_comments(source: &str) -> String {
    let mut rest = source.trim_start();
    let stripped_prefix_len = source.len() - rest.len();
    let mut removed = false;

    loop {
        if let Some(after) = rest.strip_prefix("/*") {
            let Some(end) = after.find("*/") else {
                break;
            };
            rest = after[end + "*/".len()..].trim_start();
            removed = true;
            continue;
        }

        if let Some(after) = rest.strip_prefix("//") {
            let Some(end) = after.find('\n') else {
                return String::new();
            };
            rest = after[end + 1..].trim_start();
            removed = true;
            continue;
        }

        break;
    }

    if removed {
        rest.to_string()
    } else if stripped_prefix_len == 0 {
        source.to_string()
    } else {
        source[stripped_prefix_len..].to_string()
    }
}

fn hoist_viewpu_params_interfaces(source: &str) -> String {
    if !source.contains("_Params") || !source.contains("extends ViewPU") {
        return source.to_string();
    }

    let mut interfaces = Vec::new();
    let mut body = String::with_capacity(source.len());
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if trimmed.starts_with("interface ") && trimmed.contains("_Params ") {
            let mut block = String::new();
            block.push_str(line);
            index += 1;
            while index < lines.len() {
                let block_line = lines[index];
                block.push_str(block_line);
                index += 1;
                if block_line.trim() == "}" {
                    break;
                }
            }
            interfaces.push(block);
            continue;
        }

        body.push_str(line);
        index += 1;
    }

    if interfaces.is_empty() {
        return source.to_string();
    }

    interfaces.reverse();
    let mut out = String::with_capacity(source.len());
    if let Some(after_shim) = body
        .find("}\n")
        .filter(|_| body.starts_with("if (!(\"finalizeConstruction\" in ViewPU.prototype)) {\n"))
    {
        let split = after_shim + "}\n".len();
        out.push_str(&body[..split]);
        for interface in &interfaces {
            out.push_str(interface);
        }
        out.push_str(&body[split..]);
    } else {
        for interface in &interfaces {
            out.push_str(interface);
        }
        out.push_str(&body);
    }
    out
}
