const std = @import("std");
const parser = @import("parser");

const module_record = parser.semantic.module_record;
const SymbolId = parser.traverser.semantic.SymbolId;
const SymbolTable = parser.traverser.semantic.SymbolTable;

const MAGIC: u32 = 0x414d4b59; // "YKMA", little-endian.
const VERSION: u32 = 1;

const STATUS_OK: u32 = 0;
const STATUS_INVALID_INPUT: u32 = 1;
const STATUS_PARSE_DIAGNOSTIC: u32 = 2;
const STATUS_ANALYZE_FAILED: u32 = 3;
const STATUS_OUT_OF_MEMORY: u32 = 4;

const LANG_JS: u32 = 0;
const LANG_TS: u32 = 1;
const LANG_DTS: u32 = 2;
const LANG_ARKUI: u32 = 3;

const KIND_TYPE: u32 = 0;
const KIND_VALUE: u32 = 1;

const EDGE_LOCAL: u32 = 0;
const EDGE_REEXPORT_NAMED: u32 = 1;
const EDGE_REEXPORT_STAR: u32 = 2;

const IMPORT_ENTRY_U32S: usize = 7;
const EXPORT_ENTRY_U32S: usize = 11;
const HEADER_U32S: usize = 5;

pub const AnalysisBuffer = extern struct {
    ptr: ?[*]u8,
    len: usize,
    status: u32,
};

export fn arkdown_yuku_module_analyze(
    source_ptr: ?[*]const u8,
    source_len: usize,
    lang_tag: u32,
) AnalysisBuffer {
    if (source_ptr == null) return failed(STATUS_INVALID_INPUT);
    const source = source_ptr.?[0..source_len];
    return analyze(source, langFromTag(lang_tag)) catch |err| switch (err) {
        error.OutOfMemory => failed(STATUS_OUT_OF_MEMORY),
        error.ParseDiagnostic => failed(STATUS_PARSE_DIAGNOSTIC),
        error.AnalyzeFailed => failed(STATUS_ANALYZE_FAILED),
    };
}

export fn arkdown_yuku_module_analysis_free(ptr: ?[*]u8, len: usize) void {
    if (ptr == null or len == 0) return;
    std.heap.smp_allocator.free(ptr.?[0..len]);
}

fn failed(status: u32) AnalysisBuffer {
    return .{ .ptr = null, .len = 0, .status = status };
}

const AnalyzeError = error{
    OutOfMemory,
    ParseDiagnostic,
    AnalyzeFailed,
};

fn analyze(source: []const u8, lang: parser.ast.Lang) AnalyzeError!AnalysisBuffer {
    var tree = parser.parse(std.heap.smp_allocator, source, .{
        .source_type = .module,
        .lang = lang,
        .preserve_parens = true,
        .allow_return_outside_function = false,
        .comments = .none,
    }) catch return error.AnalyzeFailed;
    defer tree.deinit();

    if (tree.diagnostics.items.len != 0) return error.ParseDiagnostic;

    var semantic = parser.semantic.analyze(&tree) catch return error.AnalyzeFailed;
    semantic.symbol_table.resolveAll(semantic.scope_tree) catch return error.AnalyzeFailed;

    const records = parser.semantic.module_record.collect(
        &tree,
        &semantic.symbol_table,
    ) catch return error.AnalyzeFailed;

    var serializer = Serializer{};
    defer serializer.deinit();

    for (records.imports) |record| {
        try serializer.addImport(&tree, &semantic.symbol_table, record);
    }
    for (records.exports) |record| {
        try serializer.addExport(&tree, &semantic.symbol_table, record);
    }

    return try serializer.finish();
}

fn langFromTag(tag: u32) parser.ast.Lang {
    return switch (tag) {
        LANG_TS => .ts,
        LANG_DTS => .dts,
        LANG_ARKUI => .arkui,
        else => .js,
    };
}

const StringRange = struct {
    start: u32,
    len: u32,
};

const PackedImport = struct {
    local: StringRange,
    imported: StringRange,
    specifier: StringRange,
    kind: u32,
};

const PackedExport = struct {
    edge: u32,
    kind: u32,
    local_is_import: u32,
    name: StringRange,
    local: StringRange,
    from: StringRange,
    specifier: StringRange,
};

const Serializer = struct {
    imports: std.ArrayList(PackedImport) = .empty,
    exports: std.ArrayList(PackedExport) = .empty,
    strings: std.ArrayList(u8) = .empty,

    fn deinit(self: *Serializer) void {
        const allocator = std.heap.smp_allocator;
        self.imports.deinit(allocator);
        self.exports.deinit(allocator);
        self.strings.deinit(allocator);
    }

    fn addImport(
        self: *Serializer,
        tree: *const parser.ast.Tree,
        table: *const SymbolTable,
        record: module_record.ImportRecord,
    ) error{OutOfMemory}!void {
        if (record.symbol == .none) return;
        if (record.name_kind != .named and record.name_kind != .star) return;

        const symbol = table.symbols[@intFromEnum(record.symbol)];
        const imported = switch (record.name_kind) {
            .named => tree.string(record.name),
            .star => "*",
            else => unreachable,
        };

        try self.imports.append(std.heap.smp_allocator, .{
            .local = try self.addString(tree.string(symbol.name)),
            .imported = try self.addString(imported),
            .specifier = try self.addString(tree.string(record.specifier)),
            .kind = if (record.kind == .type) KIND_TYPE else KIND_VALUE,
        });
    }

    fn addExport(
        self: *Serializer,
        tree: *const parser.ast.Tree,
        table: *const SymbolTable,
        record: module_record.ExportRecord,
    ) error{OutOfMemory}!void {
        switch (record.name_kind) {
            .named => {
                if (record.specifier.len() != 0 and record.from_kind == .named) {
                    try self.exports.append(std.heap.smp_allocator, .{
                        .edge = EDGE_REEXPORT_NAMED,
                        .kind = if (record.kind == .type) KIND_TYPE else KIND_VALUE,
                        .local_is_import = 0,
                        .name = try self.addString(tree.string(record.name)),
                        .local = .{ .start = 0, .len = 0 },
                        .from = try self.addString(tree.string(record.from_name)),
                        .specifier = try self.addString(tree.string(record.specifier)),
                    });
                    return;
                }

                const local = self.localSymbol(table, record.symbol);
                try self.exports.append(std.heap.smp_allocator, .{
                    .edge = EDGE_LOCAL,
                    .kind = self.bindingKind(tree, record, local),
                    .local_is_import = if (local) |symbol| @intFromBool(symbol.flags.import) else 0,
                    .name = try self.addString(tree.string(record.name)),
                    .local = if (local) |symbol| try self.addString(tree.string(symbol.name)) else .{ .start = 0, .len = 0 },
                    .from = .{ .start = 0, .len = 0 },
                    .specifier = .{ .start = 0, .len = 0 },
                });
            },
            .star => {
                try self.exports.append(std.heap.smp_allocator, .{
                    .edge = EDGE_REEXPORT_STAR,
                    .kind = if (record.kind == .type) KIND_TYPE else KIND_VALUE,
                    .local_is_import = 0,
                    .name = .{ .start = 0, .len = 0 },
                    .local = .{ .start = 0, .len = 0 },
                    .from = .{ .start = 0, .len = 0 },
                    .specifier = try self.addString(tree.string(record.specifier)),
                });
            },
            else => {},
        }
    }

    fn finish(self: *Serializer) error{OutOfMemory}!AnalysisBuffer {
        var bytes = std.ArrayList(u8).empty;
        errdefer bytes.deinit(std.heap.smp_allocator);

        try self.writeHeader(&bytes);
        for (self.imports.items) |entry| {
            try self.writeImport(&bytes, entry);
        }
        for (self.exports.items) |entry| {
            try self.writeExport(&bytes, entry);
        }
        try bytes.appendSlice(std.heap.smp_allocator, self.strings.items);

        const owned = try bytes.toOwnedSlice(std.heap.smp_allocator);
        return .{ .ptr = owned.ptr, .len = owned.len, .status = STATUS_OK };
    }

    fn addString(self: *Serializer, value: []const u8) error{OutOfMemory}!StringRange {
        if (value.len == 0) return .{ .start = 0, .len = 0 };
        const start = self.strings.items.len;
        try self.strings.appendSlice(std.heap.smp_allocator, value);
        return .{
            .start = @intCast(start),
            .len = @intCast(value.len),
        };
    }

    fn localSymbol(
        _: *Serializer,
        table: *const SymbolTable,
        symbol_id: SymbolId,
    ) ?parser.traverser.semantic.Symbol {
        if (symbol_id == .none) return null;
        return table.symbols[@intFromEnum(symbol_id)];
    }

    fn bindingKind(
        _: *Serializer,
        tree: *const parser.ast.Tree,
        record: module_record.ExportRecord,
        symbol: ?parser.traverser.semantic.Symbol,
    ) u32 {
        if (defaultTypeExportKind(tree, record)) |kind| return kind;
        if (record.kind == .type) {
            if (declaredRuntimeExportKind(tree, record)) |kind| return kind;
            return KIND_TYPE;
        }
        if (symbol) |local| {
            if (hasRuntimeBinding(local.flags)) return KIND_VALUE;
            if (local.flags.interface or
                local.flags.type_alias or
                local.flags.type_parameter or
                local.flags.const_enum or
                local.flags.type_import)
            {
                return KIND_TYPE;
            }
        }
        return KIND_VALUE;
    }

    fn writeHeader(self: *Serializer, bytes: *std.ArrayList(u8)) error{OutOfMemory}!void {
        try writeU32(bytes, MAGIC);
        try writeU32(bytes, VERSION);
        try writeU32(bytes, @intCast(self.imports.items.len));
        try writeU32(bytes, @intCast(self.exports.items.len));
        try writeU32(bytes, @intCast(self.strings.items.len));
    }

    fn writeImport(_: *Serializer, bytes: *std.ArrayList(u8), entry: PackedImport) error{OutOfMemory}!void {
        try writeRange(bytes, entry.local);
        try writeRange(bytes, entry.imported);
        try writeRange(bytes, entry.specifier);
        try writeU32(bytes, entry.kind);
    }

    fn writeExport(_: *Serializer, bytes: *std.ArrayList(u8), entry: PackedExport) error{OutOfMemory}!void {
        try writeU32(bytes, entry.edge);
        try writeU32(bytes, entry.kind);
        try writeU32(bytes, entry.local_is_import);
        try writeRange(bytes, entry.name);
        try writeRange(bytes, entry.local);
        try writeRange(bytes, entry.from);
        try writeRange(bytes, entry.specifier);
    }
};

fn hasRuntimeBinding(flags: parser.traverser.semantic.Symbol.Flags) bool {
    return flags.function_scoped_var or
        flags.block_scoped_var or
        flags.function or
        flags.class or
        flags.regular_enum or
        flags.value_module or
        flags.import or
        (flags.namespace_module and !flags.ambient);
}

fn defaultTypeExportKind(tree: *const parser.ast.Tree, record: module_record.ExportRecord) ?u32 {
    if (!std.mem.eql(u8, tree.string(record.name), "default")) return null;
    if (record.node == .null) return null;
    const data = tree.data(record.node);
    if (data != .export_default_declaration) return null;
    const declaration = data.export_default_declaration.declaration;
    return switch (tree.data(declaration)) {
        .ts_interface_declaration, .ts_type_alias_declaration => KIND_TYPE,
        else => null,
    };
}

fn declaredRuntimeExportKind(tree: *const parser.ast.Tree, record: module_record.ExportRecord) ?u32 {
    if (record.node == .null) return null;
    const data = tree.data(record.node);
    if (data != .export_named_declaration) return null;
    const declaration = data.export_named_declaration.declaration;
    if (declaration == .null) return null;
    return declarationBindingKind(tree, declaration);
}

fn declarationBindingKind(tree: *const parser.ast.Tree, declaration: parser.ast.NodeIndex) ?u32 {
    return switch (tree.data(declaration)) {
        .function, .class, .variable_declaration => KIND_VALUE,
        .ts_enum_declaration => |decl| if (decl.is_const) KIND_TYPE else KIND_VALUE,
        .ts_type_alias_declaration, .ts_interface_declaration => KIND_TYPE,
        else => null,
    };
}

fn writeRange(bytes: *std.ArrayList(u8), range: StringRange) error{OutOfMemory}!void {
    try writeU32(bytes, range.start);
    try writeU32(bytes, range.len);
}

fn writeU32(bytes: *std.ArrayList(u8), value: u32) error{OutOfMemory}!void {
    var raw: [4]u8 = undefined;
    std.mem.writeInt(u32, &raw, value, .little);
    try bytes.appendSlice(std.heap.smp_allocator, &raw);
}

comptime {
    std.debug.assert(HEADER_U32S == 5);
    std.debug.assert(IMPORT_ENTRY_U32S == 7);
    std.debug.assert(EXPORT_ENTRY_U32S == 11);
}
