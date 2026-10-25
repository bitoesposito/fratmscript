//! Semantic checks over the AST, run after parsing.
//!
//! Today this reports names declared twice in the same scope and
//! `piglie`, `aspett`, `rompe` and `salta` outside the construct they need,
//! which JS would only reject when the generated code is loaded, plus
//! imports of helpers the `fratm` library does not have.

use std::collections::HashMap;

use crate::lexer::Span;
use crate::parser::*;

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticError {
    pub message: String,
    pub span: Span,
}

#[derive(Clone, Copy, PartialEq)]
enum Binding {
    /// `tien`, `chist è`, `na famiglie`, `chiamm`
    Lexical,
    /// `facc`: may be redeclared by another `facc`, as in JS
    Function,
    /// Function and catch parameters
    Param,
}

/// Returns the first redeclaration found, in source order.
pub fn check(program: &Program) -> Result<(), SemanticError> {
    let mut checker = Checker {
        scopes: vec![Scope::default()],
        context: Context {
            allows_await: true,
            ..Context::default()
        },
        labels: Vec::new(),
    };
    checker.statements(&program.statements);
    match checker.scopes.pop().and_then(|s| s.error) {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

struct Checker {
    scopes: Vec<Scope>,
    context: Context,
    /// Labels around the statement being checked, and whether each one names
    /// a loop (only those work with `salta`). Each function starts with none.
    labels: Vec<(String, bool)>,
}

/// Where the statement being checked sits, for `piglie`, `aspett`, `rompe`
/// and `salta`. Each function starts a fresh context.
#[derive(Clone, Copy, Default)]
struct Context {
    in_function: bool,
    /// `mo vir` function, or the top level (modules allow `await` there)
    allows_await: bool,
    loops: usize,
    switches: usize,
}

#[derive(Default)]
struct Scope {
    names: HashMap<String, (Binding, usize)>,
    error: Option<SemanticError>,
}

impl Checker {
    /// Records an error unless the current scope already has one.
    fn fail(&mut self, error: SemanticError) {
        let scope = self.scopes.last_mut().expect("a scope is always open");
        if scope.error.is_none() {
            scope.error = Some(error);
        }
    }

    fn declare(&mut self, name: &str, binding: Binding, span: Span) {
        let scope = self.scopes.last_mut().expect("a scope is always open");
        if scope.error.is_some() {
            return;
        }
        if let Some(&(previous, line)) = scope.names.get(name) {
            let allowed = matches!(
                (previous, binding),
                (Binding::Function, Binding::Function) | (Binding::Param, Binding::Function)
            );
            if !allowed {
                scope.error = Some(SemanticError {
                    message: format!("'{}' is already declared (line {})", name, line),
                    span,
                });
            }
            return;
        }
        scope.names.insert(name.to_string(), (binding, span.line));
    }

    /// Runs `f` in a new scope and moves its error, if any, to the outer one.
    fn scoped(&mut self, f: impl FnOnce(&mut Self)) {
        self.scopes.push(Scope::default());
        f(self);
        let inner = self.scopes.pop().and_then(|s| s.error);
        let outer = self
            .scopes
            .last_mut()
            .expect("the program scope stays open");
        if outer.error.is_none() {
            outer.error = inner;
        }
    }

    fn block(&mut self, statements: &[Statement]) {
        self.scoped(|c| c.statements(statements));
    }

    fn statements(&mut self, statements: &[Statement]) {
        for s in statements {
            self.statement(s);
        }
    }

    fn function(&mut self, params: &[Pattern], body: &[Statement], is_async: bool) {
        self.in_function(is_async, |c| {
            c.scoped(|c| {
                c.params(params);
                c.statements(body);
            })
        });
    }

    /// Runs `f` with the context of a new function body.
    fn in_function(&mut self, is_async: bool, f: impl FnOnce(&mut Self)) {
        let context = Context {
            in_function: true,
            allows_await: is_async,
            loops: 0,
            switches: 0,
        };
        self.with_context(context, f);
    }

    fn with_context(&mut self, context: Context, f: impl FnOnce(&mut Self)) {
        let outer = std::mem::replace(&mut self.context, context);
        let outer_labels = std::mem::take(&mut self.labels);
        f(self);
        self.context = outer;
        self.labels = outer_labels;
    }

    /// Checks a loop body, where `rompe` and `salta` are allowed.
    fn loop_body(&mut self, body: &[Statement]) {
        self.context.loops += 1;
        self.block(body);
        self.context.loops -= 1;
    }

    /// `rompe x` / `salta x` without a label `x:` around them (on a loop, for `salta`).
    fn unknown_label(&mut self, keyword: &str, label: &str, span: Span) {
        self.fail(SemanticError {
            message: format!("'{} {}': no enclosing label '{}'", keyword, label, label),
            span,
        });
    }

    fn misplaced(&mut self, keyword: &str, span: Span) {
        self.fail(SemanticError {
            message: format!("'{}' is not allowed here", keyword),
            span,
        });
    }

    fn class_members(&mut self, members: &[ClassMember]) {
        for member in members {
            match member {
                ClassMember::Method(m) => {
                    self.property_key(&m.key);
                    self.function(&m.params, &m.body, m.is_async);
                }
                ClassMember::Field { key, value, .. } => {
                    self.property_key(key);
                    if let Some(value) = value {
                        self.expression(value);
                    }
                }
                // Not a function: `piglie` and `aspett` are not allowed in it
                ClassMember::StaticBlock { body, .. } => {
                    self.with_context(Context::default(), |c| c.block(body))
                }
            }
        }
    }

    fn params(&mut self, params: &[Pattern]) {
        for p in params {
            self.pattern(p, Binding::Param);
        }
        // Defaults may hold arrows with their own bodies
        for p in params {
            self.pattern_defaults(p);
        }
    }

    /// Declares every name bound by `pattern`.
    fn pattern(&mut self, pattern: &Pattern, binding: Binding) {
        match pattern {
            Pattern::Identifier { name, span } => self.declare(name, binding, *span),
            Pattern::Array { elements, .. } => {
                for p in elements.iter().flatten() {
                    self.pattern(p, binding);
                }
            }
            Pattern::Object { properties, .. } => {
                for p in properties {
                    match p {
                        ObjectPatternProperty::Property { value, .. } => {
                            self.pattern(value, binding)
                        }
                        ObjectPatternProperty::Rest(p) => self.pattern(p, binding),
                    }
                }
            }
            Pattern::Default { target, .. } => self.pattern(target, binding),
            Pattern::Rest { argument } => self.pattern(argument, binding),
        }
    }

    fn pattern_defaults(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier { .. } => {}
            Pattern::Array { elements, .. } => {
                for p in elements.iter().flatten() {
                    self.pattern_defaults(p);
                }
            }
            Pattern::Object { properties, .. } => {
                for p in properties {
                    match p {
                        ObjectPatternProperty::Property { key, value, .. } => {
                            self.property_key(key);
                            self.pattern_defaults(value);
                        }
                        ObjectPatternProperty::Rest(p) => self.pattern_defaults(p),
                    }
                }
            }
            Pattern::Default { target, default } => {
                self.pattern_defaults(target);
                self.expression(default);
            }
            Pattern::Rest { argument } => self.pattern_defaults(argument),
        }
    }

    fn statement(&mut self, statement: &Statement) {
        match statement {
            Statement::VariableDecl { declarations, .. } => {
                for d in declarations {
                    self.pattern(&d.target, Binding::Lexical);
                    self.pattern_defaults(&d.target);
                    if let Some(value) = &d.value {
                        self.expression(value);
                    }
                }
            }
            Statement::FunctionDecl {
                name,
                params,
                body,
                is_async,
                span,
                ..
            } => {
                self.declare(name, Binding::Function, *span);
                self.function(params, body, *is_async);
            }
            Statement::ClassDecl {
                name,
                members,
                span,
                ..
            } => {
                self.declare(name, Binding::Lexical, *span);
                self.class_members(members);
            }
            Statement::Return { value, span } => {
                if !self.context.in_function {
                    self.misplaced("piglie", *span);
                }
                if let Some(value) = value {
                    self.expression(value);
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition);
                self.block(then_branch);
                if let Some(else_branch) = else_branch {
                    self.block(else_branch);
                }
            }
            Statement::While {
                condition, body, ..
            }
            | Statement::DoWhile {
                condition, body, ..
            } => {
                self.expression(condition);
                self.loop_body(body);
            }
            Statement::For {
                init,
                condition,
                update,
                body,
                ..
            } => self.scoped(|c| {
                if let Some(init) = init {
                    c.statement(init);
                }
                if let Some(condition) = condition {
                    c.expression(condition);
                }
                if let Some(update) = update {
                    c.expression(update);
                }
                c.loop_body(body);
            }),
            Statement::ForEach {
                binding,
                iterable,
                body,
                is_await,
                span,
                ..
            } => self.scoped(|c| {
                if *is_await && !c.context.allows_await {
                    c.misplaced("pe aspett", *span);
                }
                match binding {
                    ForBinding::Let(p) | ForBinding::Const(p) => {
                        c.pattern(p, Binding::Lexical);
                        c.pattern_defaults(p);
                    }
                    ForBinding::Existing(_) => {}
                }
                c.expression(iterable);
                c.loop_body(body);
            }),
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.expression(discriminant);
                // All the cases share one scope, as in JS
                self.context.switches += 1;
                self.scoped(|c| {
                    for case in cases {
                        if let Some(test) = &case.test {
                            c.expression(test);
                        }
                        c.statements(&case.body);
                    }
                });
                self.context.switches -= 1;
            }
            Statement::Break { label: None, span } => {
                if self.context.loops == 0 && self.context.switches == 0 {
                    self.misplaced("rompe", *span);
                }
            }
            Statement::Continue { label: None, span } => {
                if self.context.loops == 0 {
                    self.misplaced("salta", *span);
                }
            }
            Statement::Break {
                label: Some(label),
                span,
            } => {
                if !self.labels.iter().any(|(name, _)| name == label) {
                    self.unknown_label("rompe", label, *span);
                }
            }
            Statement::Continue {
                label: Some(label),
                span,
            } => {
                if !self
                    .labels
                    .iter()
                    .any(|(name, on_loop)| name == label && *on_loop)
                {
                    self.unknown_label("salta", label, *span);
                }
            }
            Statement::Labeled { label, body, .. } => {
                let on_loop = matches!(
                    **body,
                    Statement::While { .. }
                        | Statement::DoWhile { .. }
                        | Statement::For { .. }
                        | Statement::ForEach { .. }
                );
                self.labels.push((label.clone(), on_loop));
                self.statement(body);
                self.labels.pop();
            }
            Statement::Debugger { .. } => {}
            Statement::TryCatch {
                try_body,
                catch_param,
                catch_body,
                finally_body,
                ..
            } => {
                self.block(try_body);
                if let Some(catch_body) = catch_body {
                    self.scoped(|c| {
                        if let Some(param) = catch_param {
                            c.pattern(param, Binding::Param);
                            c.pattern_defaults(param);
                        }
                        c.statements(catch_body);
                    });
                }
                if let Some(finally_body) = finally_body {
                    self.block(finally_body);
                }
            }
            Statement::Throw { value, .. } => self.expression(value),
            Statement::Import {
                default,
                namespace,
                specifiers,
                source,
                span,
            } => {
                let whole = default.iter().chain(namespace.iter());
                if source == crate::stdlib::MODULE && (default.is_some() || namespace.is_some()) {
                    self.fail(SemanticError {
                        message: format!(
                            "The fratm library is imported by name: chiamm {{ {} }} da \"fratm\"",
                            crate::stdlib::names()
                        ),
                        span: *span,
                    });
                }
                for name in whole {
                    self.declare(name, Binding::Lexical, *span);
                }
                for s in specifiers {
                    if source == crate::stdlib::MODULE
                        && crate::stdlib::helper(&s.imported).is_none()
                    {
                        self.fail(SemanticError {
                            message: format!(
                                "'{}' is not in the fratm library (available: {})",
                                s.imported,
                                crate::stdlib::names()
                            ),
                            span: *span,
                        });
                    }
                    self.declare(&s.local, Binding::Lexical, *span);
                }
            }
            Statement::Export {
                declaration,
                default_value,
                ..
            } => {
                if let Some(declaration) = declaration {
                    self.statement(declaration);
                }
                if let Some(value) = default_value {
                    self.expression(value);
                }
            }
            Statement::ExportNamed { .. } | Statement::ExportAll { .. } => {}
            Statement::Expression { expression, .. } => self.expression(expression),
            Statement::Block { statements, .. } => self.block(statements),
        }
    }

    fn property_key(&mut self, key: &PropertyKey) {
        if let PropertyKey::Computed(e) = key {
            self.expression(e);
        }
    }

    /// Expressions declare nothing themselves, but arrows and object methods
    /// open function scopes.
    fn expression(&mut self, expression: &Expression) {
        match expression {
            Expression::TaggedTemplate { tag, quasi, .. } => {
                self.expression(tag);
                self.expression(quasi);
            }
            Expression::Identifier { .. }
            | Expression::Number { .. }
            | Expression::BigInt { .. }
            | Expression::String { .. }
            | Expression::Boolean { .. }
            | Expression::Null { .. }
            | Expression::Undefined { .. }
            | Expression::This { .. }
            | Expression::Super { .. }
            | Expression::MetaProperty { .. }
            | Expression::Regex { .. } => {}
            Expression::Array { elements: list, .. }
            | Expression::Sequence {
                expressions: list, ..
            }
            | Expression::Template {
                expressions: list, ..
            }
            | Expression::ConsoleLog {
                arguments: list, ..
            }
            | Expression::ConsoleWarn {
                arguments: list, ..
            }
            | Expression::ConsoleError {
                arguments: list, ..
            } => {
                for e in list {
                    self.expression(e);
                }
            }
            Expression::Object { properties, .. } => {
                for p in properties {
                    match p {
                        Property::KeyValue { key, value } => {
                            self.property_key(key);
                            self.expression(value);
                        }
                        Property::Shorthand { .. } => {}
                        Property::Method {
                            key,
                            params,
                            body,
                            is_async,
                            ..
                        } => {
                            self.property_key(key);
                            self.function(params, body, *is_async);
                        }
                        Property::Spread(e) => self.expression(e),
                    }
                }
            }
            Expression::Await { argument, span } => {
                if !self.context.allows_await {
                    self.misplaced("aspett", *span);
                }
                self.expression(argument);
            }
            Expression::Spread { argument, .. }
            | Expression::Update { argument, .. }
            | Expression::Unary {
                operand: argument, ..
            }
            | Expression::TypeOf {
                operand: argument, ..
            }
            | Expression::Delete {
                operand: argument, ..
            }
            | Expression::Import {
                source: argument, ..
            } => self.expression(argument),
            Expression::Yield { argument, .. } => {
                if let Some(argument) = argument {
                    self.expression(argument);
                }
            }
            Expression::Binary { left, right, .. } => {
                self.expression(left);
                self.expression(right);
            }
            Expression::Assignment { target, value, .. } => {
                self.expression(target);
                self.expression(value);
            }
            Expression::Call {
                callee, arguments, ..
            }
            | Expression::New {
                callee, arguments, ..
            } => {
                self.expression(callee);
                for a in arguments {
                    self.expression(a);
                }
            }
            Expression::Member {
                object, property, ..
            } => {
                self.expression(object);
                self.expression(property);
            }
            // The name of a class expression is visible only inside it
            Expression::Class {
                name,
                members,
                span,
                ..
            } => self.scoped(|c| {
                if let Some(name) = name {
                    c.declare(name, Binding::Lexical, *span);
                }
                c.class_members(members);
            }),
            // The name of a function expression is visible only inside it
            Expression::Function {
                name,
                params,
                body,
                is_async,
                span,
                ..
            } => self.scoped(|c| {
                if let Some(name) = name {
                    c.declare(name, Binding::Function, *span);
                }
                c.function(params, body, *is_async);
            }),
            Expression::ArrowFunction {
                params,
                body,
                is_async,
                ..
            } => match body {
                ArrowBody::Block(body) => self.function(params, body, *is_async),
                ArrowBody::Expression(e) => self.in_function(*is_async, |c| {
                    c.scoped(|c| {
                        c.params(params);
                        c.expression(e);
                    })
                }),
            },
            Expression::Ternary {
                condition,
                consequent,
                alternate,
                ..
            } => {
                self.expression(condition);
                self.expression(consequent);
                self.expression(alternate);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compile;

    /// The `(line, column)` of the reported redeclaration, if any.
    fn redeclared(source: &str) -> Option<(usize, usize)> {
        match compile(source, Default::default()) {
            Err(crate::errors::CompileError::SemanticError { line, column, .. }) => {
                Some((line, column))
            }
            Err(e) => panic!("unexpected error for {:?}: {}", source, e),
            Ok(_) => None,
        }
    }

    #[test]
    fn test_reports_the_second_declaration() {
        assert_eq!(redeclared("tien x = 1\ntien x = 2"), Some((2, 6)));
        assert_eq!(redeclared("chist è x = 1\ntien x = 2"), Some((2, 6)));
        assert_eq!(redeclared("tien a = 1, a = 2"), Some((1, 13)));
        assert_eq!(redeclared("tien [a, { b }] = c\ntien b = 1"), Some((2, 6)));
        assert_eq!(redeclared("facc f() { }\ntien f = 1"), Some((2, 6)));
        assert_eq!(redeclared("tien P = 1\nna famiglie P { }"), Some((2, 1)));
        assert_eq!(
            redeclared("chiamm { a } da \"m\"\ntien a = 1"),
            Some((2, 6))
        );
    }

    #[test]
    fn test_scopes_follow_js() {
        // Inner blocks, loop bodies and functions open new scopes
        assert_eq!(redeclared("tien x = 1\nsi (x) { tien x = 2 }"), None);
        assert_eq!(
            redeclared("pe (tien i = 0; i < 1; i++) { tien i = 2 }"),
            None
        );
        assert_eq!(redeclared("pe (chist è v dint vs) { tien v = 1 }"), None);
        assert_eq!(redeclared("tien x\nchist è f = () => { tien x = 1 }"), None);
        assert_eq!(redeclared("tien x\nfacc f(x) { }"), None);
        // ...but a block is still one scope
        assert_eq!(
            redeclared("si (overo) {\n tien x = 1\n tien x = 2\n}"),
            Some((3, 7))
        );
        // Parameters clash with declarations in the body, and with each other
        assert_eq!(redeclared("facc f(a) {\n tien a = 1\n}"), Some((2, 7)));
        assert_eq!(redeclared("facc f(a, a) { }"), Some((1, 11)));
        assert_eq!(
            redeclared("chist è f = (a) => { tien a = 1 }"),
            Some((1, 27))
        );
        assert_eq!(
            redeclared("pruvamm { } e si schiatta (e) { tien e = 1 }"),
            Some((1, 38))
        );
        // Every case of a vedimm shares one scope
        assert_eq!(
            redeclared("vedimm (k) {\n caso 1: tien x = 1\n caso 2: tien x = 2\n}"),
            Some((3, 15))
        );
    }

    #[test]
    fn test_function_redeclarations_allowed_by_js() {
        assert_eq!(redeclared("facc f() { }\nfacc f() { }"), None);
        assert_eq!(redeclared("facc f(g) { facc g() { } }"), None);
    }

    #[test]
    fn test_message_is_neapolitan() {
        let err = compile("tien x = 1\ntien x = 2", Default::default()).unwrap_err();
        assert!(
            err.to_string()
                .contains("'x' già esiste: l'hai dichiarato a riga 1!"),
            "{}",
            err
        );
    }

    /// The `(line, column, message)` of a misplaced keyword, if any.
    fn misplaced(source: &str) -> Option<(usize, usize, String)> {
        match compile(source, Default::default()) {
            Err(e @ crate::errors::CompileError::SemanticError { .. }) => {
                Some((e.line()?, e.column()?, e.to_string()))
            }
            Err(e) => panic!("unexpected error for {:?}: {}", source, e),
            Ok(_) => None,
        }
    }

    #[test]
    fn test_piglie_only_inside_functions() {
        let (line, column, message) = misplaced("stamm a dì(1)\npiglie 2").unwrap();
        assert_eq!((line, column), (2, 1));
        assert!(message.contains("'piglie' se po' usà sulo dint'a na funzione"));
        assert_eq!(misplaced("facc f() { piglie 1 }"), None);
        assert_eq!(misplaced("f = () => { piglie 1 }"), None);
        assert_eq!(misplaced("o = { m() { piglie 1 } }"), None);
    }

    #[test]
    fn test_aspett_only_in_async_functions_or_at_the_top() {
        assert_eq!(
            misplaced("facc f() { aspett g() }").map(|e| (e.0, e.1)),
            Some((1, 12))
        );
        assert!(misplaced("f = () => aspett g()").is_some());
        assert!(misplaced("mo vir facc f() { facc h() { aspett g() } }").is_some());
        assert!(misplaced("facc f() { pe aspett (chist è x dint y) { } }").is_some());
        assert_eq!(misplaced("x = aspett g()"), None);
        assert_eq!(misplaced("mo vir facc f() { aspett g() }"), None);
        assert_eq!(misplaced("f = mo vir () => aspett g()"), None);
        assert_eq!(
            misplaced("na famiglie A { mo vir m() { aspett g() } }"),
            None
        );
    }

    #[test]
    fn test_rompe_and_salta_only_in_loops() {
        assert_eq!(misplaced("rompe").map(|e| (e.0, e.1)), Some((1, 1)));
        assert!(misplaced("salta").is_some());
        // A function body is not inside the loop around it
        assert!(misplaced("mentre che (overo) { facc f() { rompe } }").is_some());
        assert!(misplaced("vedimm (x) { caso 1: salta }").is_some());
        assert_eq!(misplaced("vedimm (x) { caso 1: rompe }"), None);
        assert_eq!(misplaced("mentre che (overo) { rompe }"), None);
        assert_eq!(
            misplaced("pe (tien i = 0; i < 2; i++) { si (i) { salta } }"),
            None
        );
        assert_eq!(
            misplaced("pe (chist è x dint xs) { vedimm (x) { caso 1: salta } }"),
            None
        );
        assert_eq!(misplaced("primm fa { rompe } mentre che (overo)"), None);
    }

    #[test]
    fn test_function_and_class_expressions_have_their_own_context() {
        assert_eq!(misplaced("f = facc () { piglie 1 }"), None);
        assert_eq!(misplaced("f = mo vir facc () { aspett g() }"), None);
        assert!(misplaced("f = facc () { aspett g() }").is_some());
        assert!(misplaced("mentre che (overo) { f = facc () { salta } }").is_some());
        assert_eq!(misplaced("A = na famiglie { m() { piglie 1 } }"), None);
        // A static block is not a function body
        assert!(misplaced("na famiglie A { fisso { piglie 1 } }").is_some());
    }
}
