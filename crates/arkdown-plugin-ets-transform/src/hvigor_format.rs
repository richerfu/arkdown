pub(crate) fn format_hvigor_standard_ts(source: &str) -> String {
    let mut lines = Vec::new();
    let mut iter = source.lines().peekable();

    while let Some(line) = iter.next() {
        if line.trim().is_empty() {
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.ends_with('(') && trimmed.contains('.') {
            let indent = hvigor_indent(line);
            let mut args = Vec::new();
            let mut consumed = Vec::new();
            let mut closed = false;
            while let Some(arg_line) = iter.peek().copied() {
                let arg = arg_line.trim();
                if arg == ");" {
                    iter.next();
                    closed = true;
                    break;
                }
                if arg.is_empty() {
                    break;
                }
                consumed.push(arg_line);
                args.push(arg.trim_end_matches(',').to_string());
                iter.next();
            }
            if closed && !args.is_empty() {
                let prefix = trimmed.trim_end_matches('(');
                lines.push(format!("{indent}{prefix}({});", args.join(", ")));
                continue;
            }
            lines.push(format!("{}{}", hvigor_indent(line), trimmed));
            for consumed_line in consumed {
                lines.push(format!(
                    "{}{}",
                    hvigor_indent(consumed_line),
                    consumed_line.trim_start()
                ));
            }
            continue;
        }

        lines.push(format!("{}{}", hvigor_indent(line), trimmed));
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn hvigor_indent(line: &str) -> String {
    let spaces = line.bytes().take_while(|&b| b == b' ').count();
    " ".repeat(spaces * 2)
}

pub(crate) fn module_name_from_id(id: &str) -> Option<String> {
    id.split("/src/main/ets/").next().and_then(|prefix| {
        if prefix == id || prefix.is_empty() {
            None
        } else {
            prefix.rsplit('/').next().map(str::to_string)
        }
    })
}
