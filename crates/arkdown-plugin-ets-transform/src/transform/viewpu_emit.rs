//! ViewPU source emission helpers.
//!
//! This module is the transition boundary between ArkUI lowering decisions and
//! textual output. The lowering code should call these helpers for generated
//! ViewPU statements instead of assembling those statements inline.

use std::fmt::{self, Write};

pub(crate) struct ViewpuWriter<'a> {
    out: &'a mut String,
}

impl<'a> ViewpuWriter<'a> {
    pub(crate) fn new(out: &'a mut String) -> Self {
        Self { out }
    }

    pub(crate) fn line(&mut self, indent: &str, args: fmt::Arguments<'_>) {
        self.out.push_str(indent);
        self.out
            .write_fmt(args)
            .expect("writing to String cannot fail");
        self.out.push('\n');
    }

    pub(crate) fn observe_component_creation_start(&mut self, indent: &str, view_context: &str) {
        self.line(
            indent,
            format_args!(
                "{view_context}.observeComponentCreation2((elmtId, isInitialRender) => {{"
            ),
        );
    }

    pub(crate) fn component_create(
        &mut self,
        indent: &str,
        component_name: &str,
        create_method: &str,
        args: &str,
    ) {
        self.line(
            indent,
            format_args!("{component_name}.{create_method}({args});"),
        );
    }

    pub(crate) fn component_attribute(
        &mut self,
        indent: &str,
        component_name: &str,
        attr_name: &str,
        attr_args: &str,
    ) {
        self.line(
            indent,
            format_args!("{component_name}.{attr_name}({attr_args});"),
        );
    }

    pub(crate) fn observe_component_creation_end(&mut self, indent: &str, component_name: &str) {
        self.line(indent, format_args!("}}, {component_name});"));
    }

    pub(crate) fn component_pop(&mut self, indent: &str, component_name: &str) {
        self.line(indent, format_args!("{component_name}.pop();"));
    }
}
