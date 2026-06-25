const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const yuku_root = b.option([]const u8, "yuku-root", "local yuku checkout") orelse
        @panic("missing -Dyuku-root");

    const util_module = b.createModule(.{
        .root_source_file = .{ .cwd_relative = b.pathJoin(&.{ yuku_root, "src/util/root.zig" }) },
        .target = target,
        .optimize = optimize,
    });

    const codegen_options = b.addOptions();
    codegen_options.addOption(bool, "source_maps", false);

    const parser_module = b.createModule(.{
        .root_source_file = .{ .cwd_relative = b.pathJoin(&.{ yuku_root, "src/parser/root.zig" }) },
        .target = target,
        .optimize = optimize,
    });
    parser_module.addImport("util", util_module);
    parser_module.addImport("codegen_options", codegen_options.createModule());

    const module = b.createModule(.{
        .root_source_file = b.path("yuku_module_analyzer.zig"),
        .target = target,
        .optimize = optimize,
    });
    module.addImport("parser", parser_module);

    const lib = b.addLibrary(.{
        .name = "arkdown_yuku_module_analyzer",
        .root_module = module,
        .linkage = .static,
    });
    b.installArtifact(lib);
}
