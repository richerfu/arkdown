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

fn dump_class_methods(bytes: &[u8], class_off: usize) {
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
    println!("methods for {class_name}:");
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
        let code = code_off
            .map(|off| format!(" code=0x{off:05x}"))
            .unwrap_or_default();
        let debug = debug_off
            .map(|off| format!(" debug=0x{off:05x}"))
            .unwrap_or_default();
        println!(
            "  [{i:02}] 0x{method_off:05x} access=0x{access:x}{code}{debug} tags={} {name}",
            pos - tag_start
        );
    }
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

fn print_file(path: &Path) {
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

    println!("class_index:");
    let mut class_offsets = Vec::new();
    for i in 0..num_classes as usize {
        let off = u32_at(&bytes, class_idx_off + i * 4) as usize;
        class_offsets.push(off);
        let name = read_string(&bytes, off)
            .map(|(s, _)| s)
            .unwrap_or_else(|| "<non-ascii-or-invalid>".to_string());
        println!("  [{i:02}] 0x{off:05x} {name}");
    }
    for off in class_offsets {
        dump_class_methods(&bytes, off);
    }

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

    if idx_sect_off + 40 <= bytes.len() {
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

    if num_lnp > 0 && lnp_idx_off + num_lnp as usize * 4 <= bytes.len() {
        println!("lnp_index:");
        for i in 0..num_lnp as usize {
            let off = u32_at(&bytes, lnp_idx_off + i * 4);
            println!("  [{i:02}] 0x{off:05x}");
        }
    }
}

fn main() {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: cargo run -p arkdown-abc --example abc_inspect -- <file.abc>...");
        std::process::exit(2);
    }
    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_file(Path::new(path));
    }
}
