//! Transformer / Transpiler
//!
//! References:
//! * <https://www.typescriptlang.org/tsconfig#target>
//! * <https://babel.dev/docs/presets>
//! * <https://github.com/microsoft/TypeScript/blob/main/src/compiler/transformer.ts>

mod context;
mod function;
mod inferer;
mod transform;

use std::{path::Path, rc::Rc};

use context::{Ctx, TransformDtsCtx};
use oxc_allocator::{Allocator, Box};
use oxc_ast::Trivias;
#[allow(clippy::wildcard_imports)]
use oxc_ast::{ast::*, Visit};
use oxc_codegen::{Codegen, CodegenOptions, Context, Gen};
use oxc_diagnostics::{Error, OxcDiagnostic};
use oxc_span::{GetSpan, SPAN};
use oxc_syntax::scope::ScopeFlags;

pub struct TransformerDts<'a> {
    ctx: Ctx<'a>,
    codegen: Codegen<'a, false>,
}

impl<'a> TransformerDts<'a> {
    pub fn new(
        allocator: &'a Allocator,
        source_path: &Path,
        source_text: &'a str,
        trivias: Trivias,
    ) -> Self {
        let codegen = Codegen::new(
            &source_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
            source_text,
            trivias,
            CodegenOptions::default().with_typescript(true),
        );

        let ctx = Rc::new(TransformDtsCtx::new(allocator));

        Self { ctx, codegen }
    }

    /// # Errors
    ///
    /// Returns `Vec<Error>` if any errors were collected during the transformation.
    pub fn build(mut self, program: &Program<'a>) -> Result<String, std::vec::Vec<Error>> {
        self.visit_program(program);

        let errors = self.ctx.take_errors();
        if errors.is_empty() {
            Ok(self.codegen.into_source_text())
        } else {
            Err(errors)
        }
    }
}

impl<'a> TransformerDts<'a> {
    pub fn modifiers_declare(&self) -> Modifiers<'a> {
        Modifiers::new(
            self.ctx.ast.new_vec_single(Modifier { span: SPAN, kind: ModifierKind::Declare }),
        )
    }

    pub fn transform_function(&mut self, func: &Function<'a>) -> Box<'a, Function<'a>> {
        if func.modifiers.is_contains_declare() {
            self.ctx.ast.alloc(self.ctx.ast.copy(func))
        } else {
            let return_type = self.infer_function_return_type(func);
            let params = self.transform_formal_parameters(&func.params);
            self.ctx.ast.function(
                func.r#type,
                func.span,
                self.ctx.ast.copy(&func.id),
                func.generator,
                func.r#async,
                self.ctx.ast.copy(&func.this_param),
                params,
                None,
                self.ctx.ast.copy(&func.type_parameters),
                return_type,
                self.modifiers_declare(),
            )
        }
    }

    pub fn transform_variable_declaration(
        &self,
        decl: &VariableDeclaration<'a>,
    ) -> Option<Box<'a, VariableDeclaration<'a>>> {
        if decl.modifiers.is_contains_declare() {
            None
        } else {
            let declarations = self.ctx.ast.new_vec_from_iter(
                decl.declarations
                    .iter()
                    .map(|declarator| self.transform_variable_declarator(declarator)),
            );
            Some(self.ctx.ast.variable_declaration(
                decl.span,
                decl.kind,
                declarations,
                self.modifiers_declare(),
            ))
        }
    }

    pub fn transform_variable_declarator(
        &self,
        decl: &VariableDeclarator<'a>,
    ) -> VariableDeclarator<'a> {
        let mut binding_type = None;
        let mut init = None;
        if decl.id.type_annotation.is_none() {
            if let Some(init_expr) = &decl.init {
                // if kind is const and it doesn't need to infer type from expression
                if decl.kind.is_const() && !Self::is_need_to_infer_type_from_expression(init_expr) {
                    init = Some(self.ctx.ast.copy(init_expr));
                } else {
                    // otherwise, we need to infer type from expression
                    binding_type = self.infer_type_from_expression(init_expr);
                }
            } else {
                // has not type annotation and no init, we need to report error
                binding_type = Some(self.ctx.ast.ts_unknown_keyword(SPAN));
            }
        }
        let id = binding_type.map_or_else(
            || self.ctx.ast.copy(&decl.id),
            |ts_type| {
                self.ctx.ast.binding_pattern(
                    self.ctx.ast.copy(&decl.id.kind),
                    Some(self.ctx.ast.ts_type_annotation(SPAN, ts_type)),
                    decl.id.optional,
                )
            },
        );

        self.ctx.ast.variable_declarator(decl.span, decl.kind, id, init, decl.definite)
    }

    pub fn transform_using_declaration(
        &self,
        decl: &UsingDeclaration<'a>,
    ) -> Box<'a, VariableDeclaration<'a>> {
        let declarations = self.ctx.ast.new_vec_from_iter(
            decl.declarations
                .iter()
                .map(|declarator| self.transform_variable_declarator(declarator)),
        );
        self.ctx.ast.variable_declaration(
            decl.span,
            VariableDeclarationKind::Const,
            declarations,
            self.modifiers_declare(),
        )
    }

    pub fn transform_accessibility(
        &self,
        accessibility: Option<TSAccessibility>,
    ) -> Option<TSAccessibility> {
        if accessibility.is_none() || accessibility.is_some_and(|a| a == TSAccessibility::Public) {
            None
        } else {
            accessibility
        }
    }

    pub fn report_property_key(&self, key: &PropertyKey<'a>, computed: bool) -> bool {
        if computed
            && !matches!(
                key,
                PropertyKey::StringLiteral(_)
                    | PropertyKey::NumericLiteral(_)
                    | PropertyKey::BigintLiteral(_)
            )
        {
            self.ctx.error(
                OxcDiagnostic::error("Computed property names on class or object literals cannot be inferred with --isolatedDeclarations.")
                .with_label(key.span())
            );
            true
        } else {
            false
        }
    }

    pub fn transform_class_declaration(&self, decl: &Class<'a>) -> Option<Box<'a, Class<'a>>> {
        if decl.is_declare() {
            return None;
        }

        let mut elements = self.ctx.ast.new_vec();
        let mut has_private_key = false;
        for element in &decl.body.body {
            match element {
                ClassElement::StaticBlock(_) => {}
                ClassElement::MethodDefinition(definition) => {
                    if self.report_property_key(&definition.key, definition.computed) {
                        return None;
                    }
                    if definition.key.is_private_identifier() {
                        has_private_key = true;
                    }
                    let function = &definition.value;
                    let params = self.transform_formal_parameters(&function.params);

                    if definition.kind.is_constructor() {
                        for (index, item) in function.params.items.iter().enumerate() {
                            // transformed params will definitely have type annotation
                            let type_annotation =
                                self.ctx.ast.copy(&params.items[index].pattern.type_annotation);

                            if item.accessibility.is_some() {
                                let Some(ident_name) = item.pattern.get_identifier() else {
                                    unreachable!()
                                };
                                let key = self.ctx.ast.property_key_identifier(
                                    IdentifierName::new(SPAN, ident_name.clone()),
                                );
                                let new_elements = self.ctx.ast.class_property(
                                    PropertyDefinitionType::PropertyDefinition,
                                    item.span,
                                    key,
                                    None,
                                    false,
                                    false,
                                    false,
                                    item.r#override,
                                    item.pattern.optional,
                                    false,
                                    item.readonly,
                                    type_annotation,
                                    self.transform_accessibility(item.accessibility),
                                    self.ctx.ast.new_vec(),
                                );
                                elements.push(new_elements);
                            }
                        }
                    }

                    let type_annotation = self.infer_function_return_type(function);

                    let value = self.ctx.ast.function(
                        FunctionType::TSEmptyBodyFunctionExpression,
                        function.span,
                        self.ctx.ast.copy(&function.id),
                        function.generator,
                        function.r#async,
                        self.ctx.ast.copy(&function.this_param),
                        params,
                        None,
                        self.ctx.ast.copy(&function.type_parameters),
                        // TODO: need to infer function type
                        type_annotation,
                        Modifiers::empty(),
                    );
                    let new_element = self.ctx.ast.class_method(
                        definition.r#type,
                        definition.span,
                        self.ctx.ast.copy(&definition.key),
                        definition.kind,
                        value,
                        definition.computed,
                        definition.r#static,
                        definition.r#override,
                        definition.optional,
                        self.transform_accessibility(definition.accessibility),
                        self.ctx.ast.new_vec(),
                    );
                    elements.push(new_element);
                }
                ClassElement::PropertyDefinition(property) => {
                    if self.report_property_key(&property.key, property.computed) {
                        return None;
                    }

                    if property.key.is_private_identifier() {
                        has_private_key = true;
                    }
                    let type_annotations = property
                        .type_annotation
                        .as_ref()
                        .map(|type_annotation| self.ctx.ast.copy(type_annotation))
                        .or_else(|| {
                            let new_type = property
                                .value
                                .as_ref()
                                .and_then(|expr| self.infer_type_from_expression(expr))
                                .unwrap_or_else(|| {
                                    // report error for has no type annotation
                                    self.ctx.ast.ts_unknown_keyword(property.span)
                                });

                            Some(self.ctx.ast.ts_type_annotation(SPAN, new_type))
                        });

                    let new_element = self.ctx.ast.class_property(
                        property.r#type,
                        property.span,
                        self.ctx.ast.copy(&property.key),
                        None,
                        property.computed,
                        property.r#static,
                        property.declare,
                        property.r#override,
                        property.optional,
                        property.definite,
                        property.readonly,
                        type_annotations,
                        self.transform_accessibility(property.accessibility),
                        self.ctx.ast.new_vec(),
                    );
                    elements.push(new_element);
                }
                ClassElement::AccessorProperty(property) => {
                    if self.report_property_key(&property.key, property.computed) {
                        return None;
                    }

                    if property.key.is_private_identifier() {
                        has_private_key = true;
                    }
                    // FIXME: missing many fields
                    let new_element = self.ctx.ast.accessor_property(
                        property.r#type,
                        property.span,
                        self.ctx.ast.copy(&property.key),
                        None,
                        property.computed,
                        property.r#static,
                        self.ctx.ast.new_vec(),
                    );
                    elements.push(new_element);
                }
                ClassElement::TSIndexSignature(_) => elements.push(self.ctx.ast.copy(element)),
            }
        }

        if has_private_key {
            // <https://github.com/microsoft/TypeScript/blob/64d2eeea7b9c7f1a79edf42cb99f302535136a2e/src/compiler/transformers/declarations.ts#L1699-L1709>
            // When the class has at least one private identifier, create a unique constant identifier to retain the nominal typing behavior
            // Prevents other classes with the same public members from being used in place of the current class
            let ident = self
                .ctx
                .ast
                .property_key_private_identifier(PrivateIdentifier::new(SPAN, "private".into()));
            let r#type = PropertyDefinitionType::PropertyDefinition;
            let decorators = self.ctx.ast.new_vec();
            let new_element = self.ctx.ast.class_property(
                r#type, SPAN, ident, None, false, false, false, false, false, false, false, None,
                None, decorators,
            );
            elements.insert(0, new_element);
        }

        let body = self.ctx.ast.class_body(decl.body.span, elements);

        let modifiers = if decl.modifiers.is_contains_abstract() {
            let modifiers = self.ctx.ast.new_vec_from_iter([
                Modifier { span: SPAN, kind: ModifierKind::Declare },
                Modifier { span: SPAN, kind: ModifierKind::Abstract },
            ]);
            Modifiers::new(modifiers)
        } else {
            self.modifiers_declare()
        };

        Some(self.ctx.ast.class(
            decl.r#type,
            decl.span,
            self.ctx.ast.copy(&decl.id),
            self.ctx.ast.copy(&decl.super_class),
            body,
            self.ctx.ast.copy(&decl.type_parameters),
            self.ctx.ast.copy(&decl.super_type_parameters),
            self.ctx.ast.copy(&decl.implements),
            self.ctx.ast.new_vec(),
            modifiers,
        ))
    }

    pub fn transform_formal_parameter(
        &self,
        param: &FormalParameter<'a>,
        next_param: Option<&FormalParameter<'a>>,
    ) -> FormalParameter<'a> {
        let is_assignment_pattern = param.pattern.kind.is_assignment_pattern();
        let mut pattern =
            if let BindingPatternKind::AssignmentPattern(pattern) = &param.pattern.kind {
                self.ctx.ast.copy(&pattern.left)
            } else {
                self.ctx.ast.copy(&param.pattern)
            };

        if is_assignment_pattern || pattern.type_annotation.is_none() {
            let is_next_param_optional =
                next_param.map_or(true, |next_param| next_param.pattern.optional);
    
            let type_annotation = pattern
                .type_annotation
                .as_ref()
                .map(|type_annotation| self.ctx.ast.copy(&type_annotation.type_annotation))
                .or_else(|| {
                    // report error for has no type annotation
                    let new_type = self
                        .infer_type_from_formal_parameter(param)
                        .unwrap_or_else(|| self.ctx.ast.ts_unknown_keyword(param.span));
                    Some(new_type)
                })
                .map(|ts_type| {
                    // jf next param is not optional and current param is assignment pattern
                    // we need to add undefined to it's type
                    if !is_next_param_optional {
                        if matches!(ts_type, TSType::TSTypeReference(_)) {
                            self.ctx.error(
                                OxcDiagnostic::error("Declaration emit for this parameter requires implicitly adding undefined to it's type. This is not supported with --isolatedDeclarations.")
                                    .with_label(param.span),
                            );
                        } else if !ts_type.is_maybe_undefined() {
                            // union with undefined
                            return self.ctx.ast.ts_type_annotation(SPAN, 
                                self.ctx.ast.ts_union_type(SPAN, self.ctx.ast.new_vec_from_iter([ts_type, self.ctx.ast.ts_undefined_keyword(SPAN)]))
                            );
                        }
                    }

                    self.ctx.ast.ts_type_annotation(SPAN, ts_type)
                });

            pattern = self.ctx.ast.binding_pattern(
                self.ctx.ast.copy(&pattern.kind),
                type_annotation,
                // if it's assignment pattern, it's optional
                pattern.optional || (is_next_param_optional && is_assignment_pattern),
            );
        }

        self.ctx.ast.formal_parameter(
            param.span,
            pattern,
            None,
            param.readonly,
            false,
            self.ctx.ast.new_vec(),
        )
    }

    pub fn transform_formal_parameters(
        &self,
        params: &FormalParameters<'a>,
    ) -> Box<'a, FormalParameters<'a>> {
        if params.kind.is_signature() || (params.rest.is_none() && params.items.is_empty()) {
            return self.ctx.ast.alloc(self.ctx.ast.copy(params));
        }

        let items =
            self.ctx.ast.new_vec_from_iter(params.items.iter().enumerate().map(|(index, item)| {
                self.transform_formal_parameter(item, params.items.get(index + 1))
            }));

        if let Some(rest) = &params.rest {
            if rest.argument.type_annotation.is_none() {
                self.ctx.error(OxcDiagnostic::error(
                    "Parameter must have an explicit type annotation with --isolatedDeclarations.",
                ).with_label(rest.span));
            }
        }

        self.ctx.ast.formal_parameters(
            params.span,
            FormalParameterKind::Signature,
            items,
            self.ctx.ast.copy(&params.rest),
        )
    }
}

impl<'a> Visit<'a> for TransformerDts<'a> {
    fn visit_export_named_declaration(&mut self, export_decl: &ExportNamedDeclaration<'a>) {
        if let Some(decl) = &export_decl.declaration {
            let new_decl = match decl {
                Declaration::FunctionDeclaration(func) => {
                    Some(Declaration::FunctionDeclaration(self.transform_function(func)))
                }
                Declaration::VariableDeclaration(decl) => {
                    self.transform_variable_declaration(decl).map(Declaration::VariableDeclaration)
                }
                Declaration::UsingDeclaration(decl) => {
                    Some(Declaration::VariableDeclaration(self.transform_using_declaration(decl)))
                }
                Declaration::ClassDeclaration(decl) => {
                    self.transform_class_declaration(decl).map(Declaration::ClassDeclaration)
                }
                _ => None,
            };
            if new_decl.is_some() {
                ExportNamedDeclaration {
                    span: export_decl.span,
                    declaration: new_decl,
                    specifiers: self.ctx.ast.copy(&export_decl.specifiers),
                    source: self.ctx.ast.copy(&export_decl.source),
                    export_kind: export_decl.export_kind,
                    with_clause: self.ctx.ast.copy(&export_decl.with_clause),
                }
                .gen(&mut self.codegen, Context::empty());
            } else {
                export_decl.gen(&mut self.codegen, Context::empty());
            }
        } else {
            export_decl.gen(&mut self.codegen, Context::empty());
        }
    }

    fn visit_export_default_declaration(&mut self, decl: &ExportDefaultDeclaration<'a>) {
        decl.gen(&mut self.codegen, Context::empty());
    }

    fn visit_function(&mut self, func: &Function<'a>, _flags: Option<ScopeFlags>) {
        let func = self.transform_function(func);
        func.gen(&mut self.codegen, Context::empty());
    }

    fn visit_variable_declaration(&mut self, decl: &VariableDeclaration<'a>) {
        if let Some(decl) = self.transform_variable_declaration(decl) {
            decl.gen(&mut self.codegen, Context::empty());
        } else {
            decl.gen(&mut self.codegen, Context::empty());
        }
    }

    fn visit_using_declaration(&mut self, decl: &UsingDeclaration<'a>) {
        self.transform_using_declaration(decl).gen(&mut self.codegen, Context::empty());
    }

    fn visit_class(&mut self, decl: &Class<'a>) {
        if let Some(decl) = self.transform_class_declaration(decl) {
            decl.gen(&mut self.codegen, Context::empty());
        } else {
            decl.gen(&mut self.codegen, Context::empty());
        }
    }

    fn visit_ts_interface_declaration(&mut self, decl: &TSInterfaceDeclaration<'a>) {
        decl.gen(&mut self.codegen, Context::empty());
    }
}
