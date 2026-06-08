use oxc_ast::ast::{
    AssignmentTarget, AssignmentTargetMaybeDefault, AssignmentTargetProperty, Expression,
    SimpleAssignmentTarget,
};

pub(super) fn collect_assignment_target_names(
    target: &AssignmentTarget<'_>,
    out: &mut Vec<String>,
) {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => push_name(out, id.name.as_str()),
        AssignmentTarget::TSAsExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTarget::TSSatisfiesExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTarget::TSNonNullExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTarget::TSTypeAssertion(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTarget::ArrayAssignmentTarget(array) => {
            for element in array.elements.iter().flatten() {
                collect_assignment_target_maybe_default_names(element, out);
            }
            if let Some(rest) = &array.rest {
                collect_assignment_target_names(&rest.target, out);
            }
        }
        AssignmentTarget::ObjectAssignmentTarget(object) => {
            for property in &object.properties {
                match property {
                    AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(prop) => {
                        push_name(out, prop.binding.name.as_str());
                    }
                    AssignmentTargetProperty::AssignmentTargetPropertyProperty(prop) => {
                        collect_assignment_target_maybe_default_names(&prop.binding, out);
                    }
                }
            }
            if let Some(rest) = &object.rest {
                collect_assignment_target_names(&rest.target, out);
            }
        }
        AssignmentTarget::StaticMemberExpression(_)
        | AssignmentTarget::ComputedMemberExpression(_)
        | AssignmentTarget::PrivateFieldExpression(_) => {}
    }
}

pub(super) fn collect_simple_assignment_target_names(
    target: &SimpleAssignmentTarget<'_>,
    out: &mut Vec<String>,
) {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => push_name(out, id.name.as_str()),
        SimpleAssignmentTarget::TSAsExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        SimpleAssignmentTarget::TSNonNullExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        SimpleAssignmentTarget::TSTypeAssertion(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        SimpleAssignmentTarget::StaticMemberExpression(_)
        | SimpleAssignmentTarget::ComputedMemberExpression(_)
        | SimpleAssignmentTarget::PrivateFieldExpression(_) => {}
    }
}

fn collect_assignment_target_maybe_default_names(
    target: &AssignmentTargetMaybeDefault<'_>,
    out: &mut Vec<String>,
) {
    match target {
        AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(default) => {
            collect_assignment_target_names(&default.binding, out);
        }
        AssignmentTargetMaybeDefault::AssignmentTargetIdentifier(id) => {
            push_name(out, id.name.as_str());
        }
        AssignmentTargetMaybeDefault::TSAsExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTargetMaybeDefault::TSSatisfiesExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTargetMaybeDefault::TSNonNullExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTargetMaybeDefault::TSTypeAssertion(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        AssignmentTargetMaybeDefault::ArrayAssignmentTarget(array) => {
            for element in array.elements.iter().flatten() {
                collect_assignment_target_maybe_default_names(element, out);
            }
            if let Some(rest) = &array.rest {
                collect_assignment_target_names(&rest.target, out);
            }
        }
        AssignmentTargetMaybeDefault::ObjectAssignmentTarget(object) => {
            for property in &object.properties {
                match property {
                    AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(prop) => {
                        push_name(out, prop.binding.name.as_str());
                    }
                    AssignmentTargetProperty::AssignmentTargetPropertyProperty(prop) => {
                        collect_assignment_target_maybe_default_names(&prop.binding, out);
                    }
                }
            }
            if let Some(rest) = &object.rest {
                collect_assignment_target_names(&rest.target, out);
            }
        }
        AssignmentTargetMaybeDefault::StaticMemberExpression(_)
        | AssignmentTargetMaybeDefault::ComputedMemberExpression(_)
        | AssignmentTargetMaybeDefault::PrivateFieldExpression(_) => {}
    }
}

fn collect_assignment_expression_names(expr: &Expression<'_>, out: &mut Vec<String>) {
    match expr {
        Expression::Identifier(id) => push_name(out, id.name.as_str()),
        Expression::TSAsExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        Expression::TSSatisfiesExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        Expression::TSNonNullExpression(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        Expression::TSTypeAssertion(expr) => {
            collect_assignment_expression_names(&expr.expression, out)
        }
        _ => {}
    }
}

fn push_name(out: &mut Vec<String>, name: &str) {
    if !out.iter().any(|existing| existing == name) {
        out.push(name.to_string());
    }
}
