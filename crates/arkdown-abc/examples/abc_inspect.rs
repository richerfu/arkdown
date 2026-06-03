use std::env;
use std::path::Path;

fn u32_at(bytes: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(bytes[off..off + 4].try_into().expect("u32 in range"))
}

fn read_string(bytes: &[u8], off: usize) -> Option<(String, usize)> {
    let (prefix, prefix_len) = read_uleb(bytes, off)?;
    let len = (prefix >> 1) as usize;
    let start = off + prefix_len;
    let end = start.checked_add(len)?;
    if end >= bytes.len() || bytes[end] != 0 {
        return None;
    }
    Some((
        String::from_utf8_lossy(&bytes[start..end]).into_owned(),
        end + 1 - off,
    ))
}

fn read_uleb(bytes: &[u8], mut off: usize) -> Option<(u32, usize)> {
    let start = off;
    let mut value = 0u32;
    let mut shift = 0;
    loop {
        let byte = *bytes.get(off)?;
        off += 1;
        value |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Some((value, off - start));
        }
        shift += 7;
    }
}

fn instruction_len(opcode: u8, next: Option<u8>) -> usize {
    match opcode {
        0x00..=0x03 | 0x64 | 0x65 | 0x70 | 0xbf | 0xc0 => 1,
        0x05 | 0x09 | 0x1c | 0x29 | 0x36 | 0x3c | 0x3d | 0x44 | 0x4d | 0x4f | 0x51 | 0x60
        | 0x61 | 0x67 | 0x7c | 0x7d | 0x7e | 0xb1 | 0xc3 | 0xc4 | 0xc5 | 0xcd | 0xce => 2,
        0x2a | 0x2d | 0x3e | 0x45 | 0x50 | 0x68 | 0x77 | 0x8a | 0x8b | 0x9b | 0x4e | 0xb4
        | 0xb9 | 0xc6 => 3,
        0x0a..=0x1b | 0x25 | 0x26 | 0x27 | 0x28 => 3,
        0x1f..=0x22 => 2,
        0x06 | 0x07 | 0x08 | 0x2b | 0x2e | 0x31 | 0x32 | 0x38 | 0x3f | 0x40 | 0x42 | 0x73
        | 0x78 | 0x99 | 0xb6 | 0xba => 4,
        0x37 => 3,
        0x2c | 0x33 | 0x34 | 0x43 | 0x62 | 0x79 | 0x7a | 0x8e | 0x8f | 0x98 | 0x9a | 0x9c
        | 0xbc | 0xdb | 0xdc => 5,
        0x2f => 5,
        0x30 => 6,
        0x35 | 0x63 => 9,
        0xcf => 2,
        0x80 => 3,
        0xcb => 6,
        0xfe if next == Some(0x00) => 2,
        0xfe if matches!(next, Some(0x05 | 0x07)) => 3,
        0xfe if next == Some(0x09) => 4,
        0xfb if next == Some(0x01) => 5,
        0xfb if matches!(next, Some(0x13 | 0x14)) => 3,
        _ => 1,
    }
}

fn instruction_boundaries(bytecode: &[u8]) -> Vec<bool> {
    let mut boundaries = vec![false; bytecode.len() + 1];
    let mut pos = 0usize;
    while pos < bytecode.len() {
        boundaries[pos] = true;
        pos += instruction_len(bytecode[pos], bytecode.get(pos + 1).copied()).max(1);
    }
    if pos == bytecode.len() {
        boundaries[pos] = true;
    }
    boundaries
}

fn hex_at(bytecode: &[u8], offset: u32) -> String {
    let start = offset as usize;
    if start >= bytecode.len() {
        return "<out>".to_string();
    }
    let end = start.saturating_add(12).min(bytecode.len());
    bytecode[start..end]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn dump_code_item(bytes: &[u8], off: usize) {
    let Some((num_vregs, n)) = read_uleb(bytes, off) else {
        return;
    };
    let mut pos = off + n;
    let Some((num_args, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    let Some((code_size, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    let Some((tries_size, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    let code_end = pos.saturating_add(code_size as usize).min(bytes.len());
    let preview_end = pos.saturating_add(1024).min(code_end);
    let full_bytecode = &bytes[pos..code_end];
    let boundaries = instruction_boundaries(full_bytecode);
    let bytecode = bytes[pos..preview_end]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "       code_item vregs={num_vregs} args={num_args} code_size={code_size} tries={tries_size} bytes={bytecode}"
    );
    if tries_size > 0 {
        let mut try_pos = pos + code_size as usize;
        for i in 0..tries_size {
            let Some((start, n)) = read_uleb(bytes, try_pos) else {
                break;
            };
            try_pos += n;
            let Some((len, n)) = read_uleb(bytes, try_pos) else {
                break;
            };
            try_pos += n;
            let Some((catch_count, n)) = read_uleb(bytes, try_pos) else {
                break;
            };
            try_pos += n;
            for catch_idx in 0..catch_count {
                let Some((type_idx, n)) = read_uleb(bytes, try_pos) else {
                    break;
                };
                try_pos += n;
                let Some((handler, n)) = read_uleb(bytes, try_pos) else {
                    break;
                };
                try_pos += n;
                let Some((handler_size, n)) = read_uleb(bytes, try_pos) else {
                    break;
                };
                try_pos += n;
                let handler_end = handler + handler_size;
                println!(
                    "       try[{i}.{catch_idx}] start={start}{} [{}] end={}{} [{}] len={len} catches={catch_count} type={type_idx} handler={handler}{} [{}] handler_end={handler_end}{} [{}]",
                    if boundaries.get(start as usize).copied().unwrap_or(false) { "" } else { "!" },
                    hex_at(full_bytecode, start),
                    start + len,
                    if boundaries.get((start + len) as usize).copied().unwrap_or(false) { "" } else { "!" },
                    hex_at(full_bytecode, start + len),
                    if boundaries.get(handler as usize).copied().unwrap_or(false) { "" } else { "!" },
                    hex_at(full_bytecode, handler),
                    if boundaries.get(handler_end as usize).copied().unwrap_or(false) { "" } else { "!" },
                    hex_at(full_bytecode, handler_end),
                );
            }
        }
    }
}

fn dump_class_methods(bytes: &[u8], class_off: usize, filter: Option<&str>) {
    let Some((class_name, name_len)) = read_string(bytes, class_off) else {
        return;
    };
    let mut pos = class_off + name_len + 4;
    let Some((_, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    let Some((field_count, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    let Some((method_count, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    loop {
        let Some(tag) = bytes.get(pos).copied() else {
            return;
        };
        pos += 1;
        match tag {
            0x00 => break,
            0x02 => pos += 1,
            0x07 => pos += 4,
            _ => return,
        }
    }
    for _ in 0..field_count {
        pos += 2 + 2 + 4;
        let Some((_, n)) = read_uleb(bytes, pos) else {
            return;
        };
        pos += n;
        loop {
            let Some(tag) = bytes.get(pos).copied() else {
                return;
            };
            pos += 1;
            match tag {
                0x00 => break,
                0x01 => pos += 1,
                0x02 => pos += 4,
                _ => return,
            }
        }
    }
    if method_count == 0 {
        return;
    }
    let mut printed_header = false;
    for i in 0..method_count as usize {
        let method_off = pos;
        pos += 2 + 2;
        let name_off = u32_at(bytes, pos) as usize;
        pos += 4;
        let Some((access, n)) = read_uleb(bytes, pos) else {
            return;
        };
        pos += n;
        let name = read_string(bytes, name_off)
            .map(|(s, _)| s)
            .unwrap_or_else(|| "<bad-name>".to_string());
        let matches_filter =
            filter.is_none_or(|needle| class_name.contains(needle) || name.contains(needle));
        let mut code_off = None;
        let mut debug_off = None;
        let tag_start = pos;
        loop {
            let Some(tag) = bytes.get(pos).copied() else {
                return;
            };
            pos += 1;
            match tag {
                0x00 => break,
                0x01 => {
                    code_off = Some(u32_at(bytes, pos));
                    pos += 4;
                }
                0x02 => pos += 1,
                0x05 => {
                    debug_off = Some(u32_at(bytes, pos));
                    pos += 4;
                }
                0x06 => pos += 4,
                _ => return,
            }
        }
        if matches_filter {
            if !printed_header {
                println!("methods for {class_name}:");
                printed_header = true;
            }
            let code = code_off
                .map(|off| format!(" code=0x{off:05x}"))
                .unwrap_or_default();
            let debug = debug_off
                .map(|off| format!(" debug=0x{off:05x}"))
                .unwrap_or_default();
            println!(
                "  [{i:02}] 0x{method_off:05x} name_off=0x{name_off:05x} access=0x{access:x}{code}{debug} tags={} {name}",
                pos - tag_start
            );
            if let Some(code_off) = code_off {
                dump_code_item(bytes, code_off as usize);
            }
        }
    }
}

fn dump_class_fields(bytes: &[u8], class_off: usize, filter: Option<&str>) {
    let Some((class_name, name_len)) = read_string(bytes, class_off) else {
        return;
    };
    if !class_name.contains("@protobufjs/base64/index")
        && !filter.is_some_and(|needle| class_name.contains(needle))
    {
        return;
    }

    let mut pos = class_off + name_len + 4;
    let Some((_, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    let Some((field_count, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    let Some((method_count, n)) = read_uleb(bytes, pos) else {
        return;
    };
    pos += n;
    loop {
        let Some(tag) = bytes.get(pos).copied() else {
            return;
        };
        pos += 1;
        match tag {
            0x00 => break,
            0x02 => pos += 1,
            0x07 => pos += 4,
            _ => return,
        }
    }

    println!("fields for {class_name}:");
    for _ in 0..field_count {
        pos += 2 + 2;
        let name_off = u32_at(bytes, pos) as usize;
        pos += 4;
        let Some((_, n)) = read_uleb(bytes, pos) else {
            return;
        };
        pos += n;
        let name = read_string(bytes, name_off)
            .map(|(s, _)| s)
            .unwrap_or_else(|| "<bad-name>".to_string());
        let mut values = Vec::new();
        loop {
            let Some(tag) = bytes.get(pos).copied() else {
                return;
            };
            pos += 1;
            match tag {
                0x00 => break,
                0x01 => {
                    values.push(format!("u8({})", bytes[pos]));
                    pos += 1;
                }
                0x02 => {
                    let value = u32_at(bytes, pos);
                    values.push(format!("u32(0x{value:x})"));
                    pos += 4;
                }
                _ => return,
            }
        }
        println!("  {name} = {}", values.join(", "));
        for value in &values {
            let Some(hex) = value
                .strip_prefix("u32(0x")
                .and_then(|value| value.strip_suffix(')'))
            else {
                continue;
            };
            let Ok(off) = usize::from_str_radix(hex, 16) else {
                continue;
            };
            if name == "moduleRecordIdx" {
                dump_module_record(bytes, off);
            } else if name == "scopeNames" {
                print!("    literal 0x{off:x}");
                dump_literal_array(bytes, off);
            }
        }
    }
    println!("  methods={method_count}");
}

fn dump_string_run(bytes: &[u8], start: usize, end: usize) {
    println!("string_run 0x{start:x}..0x{end:x}:");
    let mut pos = start;
    let mut i = 0usize;
    while pos < end {
        let Some((string, size)) = read_string(bytes, pos) else {
            println!("  [{i:02}] 0x{pos:05x} <non-string>");
            break;
        };
        println!("  [{i:02}] 0x{pos:05x} {string}");
        pos += size;
        i += 1;
    }
}

fn dump_literal_array(bytes: &[u8], off: usize) {
    if off + 4 > bytes.len() {
        return;
    }
    let count = u32_at(bytes, off) as usize;
    let mut pos = off + 4;
    print!(" items=");
    let mut rendered = Vec::new();
    for _ in 0..count.min(40) {
        let Some(tag) = bytes.get(pos).copied() else {
            break;
        };
        match tag {
            0x02 if pos + 5 <= bytes.len() => {
                rendered.push(format!("int({})", u32_at(bytes, pos + 1)));
                pos += 5;
            }
            0x05 if pos + 5 <= bytes.len() => {
                let s_off = u32_at(bytes, pos + 1) as usize;
                let s = read_string(bytes, s_off)
                    .map(|(s, _)| s)
                    .unwrap_or_else(|| format!("@0x{s_off:x}"));
                rendered.push(format!("str({s})"));
                pos += 5;
            }
            0x06 if pos + 5 <= bytes.len() => {
                rendered.push(format!("method(0x{:x})", u32_at(bytes, pos + 1)));
                pos += 5;
            }
            0x09 if pos + 5 <= bytes.len() => {
                rendered.push(format!("aff({})", u32_at(bytes, pos + 1)));
                pos += 5;
            }
            0xff if pos + 2 <= bytes.len() => {
                rendered.push("null".to_string());
                pos += 2;
            }
            _ if pos + 4 <= bytes.len() => {
                let value = u32_at(bytes, pos);
                let s = read_string(bytes, value as usize).map(|(s, _)| s);
                if let Some(s) = s {
                    rendered.push(format!("rawstr({s})"));
                } else {
                    rendered.push(format!("raw32({value})"));
                }
                pos += 4;
            }
            _ => break,
        }
    }
    println!("{}", rendered.join(", "));
}

fn raw_string_at(bytes: &[u8], off: usize) -> String {
    read_string(bytes, off)
        .map(|(s, _)| s)
        .unwrap_or_else(|| format!("@0x{off:x}"))
}

fn dump_module_record(bytes: &[u8], off: usize) {
    if off + 8 > bytes.len() {
        return;
    }
    let count = u32_at(bytes, off);
    let mut pos = off + 4;
    println!("    module_record count={count}");

    let request_count = u32_at(bytes, pos) as usize;
    pos += 4;
    println!("      module_requests={request_count}");
    for i in 0..request_count {
        let request = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        println!("        [{i}] {request}");
    }

    let regular_count = u32_at(bytes, pos) as usize;
    pos += 4;
    println!("      regular_imports={regular_count}");
    for i in 0..regular_count {
        let local = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        let import = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        let request_idx = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap());
        pos += 2;
        println!("        [{i}] {local} <- {import} @ {request_idx}");
    }

    let namespace_count = u32_at(bytes, pos) as usize;
    pos += 4;
    println!("      namespace_imports={namespace_count}");
    for i in 0..namespace_count {
        let local = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        let request_idx = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap());
        pos += 2;
        println!("        [{i}] {local} @ {request_idx}");
    }

    let local_count = u32_at(bytes, pos) as usize;
    pos += 4;
    println!("      local_exports={local_count}");
    for i in 0..local_count {
        let local = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        let export = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        println!("        [{i}] {local} -> {export}");
    }

    let indirect_count = u32_at(bytes, pos) as usize;
    pos += 4;
    println!("      indirect_exports={indirect_count}");
    for i in 0..indirect_count {
        let export = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        let import = raw_string_at(bytes, u32_at(bytes, pos) as usize);
        pos += 4;
        let request_idx = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap());
        pos += 2;
        println!("        [{i}] {export} <- {import} @ {request_idx}");
    }

    let star_count = u32_at(bytes, pos) as usize;
    pos += 4;
    println!("      star_exports={star_count}");
    for i in 0..star_count {
        let request_idx = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap());
        pos += 2;
        println!("        [{i}] @ {request_idx}");
    }
}

fn print_file(path: &Path, filter: Option<&str>) {
    let bytes = std::fs::read(path).expect("read abc");
    println!("file={}", path.display());
    println!("size={}", bytes.len());
    println!("magic={:?}", &bytes[0..8]);
    println!("checksum=0x{:08x}", u32_at(&bytes, 0x08));
    println!("version=0x{:08x}", u32_at(&bytes, 0x0c));
    println!("file_size={}", u32_at(&bytes, 0x10));
    let num_classes = u32_at(&bytes, 0x1c);
    let class_idx_off = u32_at(&bytes, 0x20) as usize;
    let num_lnp = u32_at(&bytes, 0x24);
    let lnp_idx_off = u32_at(&bytes, 0x28) as usize;
    let num_lit = u32_at(&bytes, 0x2c);
    let lit_idx_off = u32_at(&bytes, 0x30) as usize;
    let num_idx_regions = u32_at(&bytes, 0x34);
    let idx_sect_off = u32_at(&bytes, 0x38) as usize;
    println!(
        "header classes={num_classes} class_idx=0x{class_idx_off:x} lnp={num_lnp} lnp_idx=0x{lnp_idx_off:x} lit={num_lit} lit_idx=0x{lit_idx_off:x} idx_regions={num_idx_regions} idx_sect=0x{idx_sect_off:x}"
    );

    if filter.is_none() {
        println!("class_index:");
    }
    let mut class_offsets = Vec::new();
    for i in 0..num_classes as usize {
        let off = u32_at(&bytes, class_idx_off + i * 4) as usize;
        class_offsets.push(off);
        let name = read_string(&bytes, off)
            .map(|(s, _)| s)
            .unwrap_or_else(|| "<non-ascii-or-invalid>".to_string());
        if filter.is_none_or(|needle| name.contains(needle)) {
            println!("  [{i:02}] 0x{off:05x} {name}");
        }
    }
    for off in class_offsets {
        dump_class_fields(&bytes, off, filter);
        dump_class_methods(&bytes, off, filter);
    }

    if filter.is_none() {
        println!("literal_index:");
        for i in 0..num_lit as usize {
            let off = u32_at(&bytes, lit_idx_off + i * 4) as usize;
            let count = if off + 4 <= bytes.len() {
                u32_at(&bytes, off)
            } else {
                0
            };
            print!("  [{i:02}] 0x{off:05x} count={count}");
            dump_literal_array(&bytes, off);
        }
    }

    if filter.is_none() && idx_sect_off + 40 <= bytes.len() {
        let start = u32_at(&bytes, idx_sect_off);
        let end = u32_at(&bytes, idx_sect_off + 4);
        let class_count = u32_at(&bytes, idx_sect_off + 8);
        let class_off = u32_at(&bytes, idx_sect_off + 12) as usize;
        let method_count = u32_at(&bytes, idx_sect_off + 16);
        let method_off = u32_at(&bytes, idx_sect_off + 20) as usize;
        println!(
            "index_header start=0x{start:x} end=0x{end:x} class_count={class_count} class_off=0x{class_off:x} method_count={method_count} method_off=0x{method_off:x}"
        );
        println!("index_classes:");
        for i in 0..class_count as usize {
            let off = u32_at(&bytes, class_off + i * 4) as usize;
            let name = read_string(&bytes, off)
                .map(|(s, _)| s)
                .unwrap_or_else(|| "<scalar-or-invalid>".to_string());
            println!("  [{i:02}] 0x{off:05x} {name}");
        }
        println!("method_index:");
        let mut first_string_off = usize::MAX;
        for i in 0..method_count as usize {
            let off = u32_at(&bytes, method_off + i * 4) as usize;
            let label = read_string(&bytes, off)
                .map(|(s, _)| s)
                .unwrap_or_else(|| "<method/literal/non-string>".to_string());
            if off > idx_sect_off && read_string(&bytes, off).is_some() {
                first_string_off = first_string_off.min(off);
            }
            println!("  [{i:02}] 0x{off:05x} {label}");
        }
        if num_lit > 0 && first_string_off != usize::MAX {
            let first_lit = (0..num_lit as usize)
                .map(|i| u32_at(&bytes, lit_idx_off + i * 4) as usize)
                .min()
                .unwrap();
            dump_string_run(&bytes, first_string_off, first_lit);
        }
    }

    if filter.is_none() && num_lnp > 0 && lnp_idx_off + num_lnp as usize * 4 <= bytes.len() {
        println!("lnp_index:");
        for i in 0..num_lnp as usize {
            let off = u32_at(&bytes, lnp_idx_off + i * 4);
            println!("  [{i:02}] 0x{off:05x}");
        }
    }
}

fn main() {
    let mut filter = None;
    let mut paths = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--filter" {
            filter = args.next();
        } else {
            paths.push(arg);
        }
    }
    if paths.is_empty() {
        eprintln!(
            "usage: cargo run -p arkdown-abc --example abc_inspect -- [--filter text] <file.abc>..."
        );
        std::process::exit(2);
    }
    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_file(Path::new(path), filter.as_deref());
    }
}
