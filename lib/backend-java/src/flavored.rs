//! Java flavor.

#![allow(unused)]

use core::errors::Result;
use core::{self, CoreFlavor, Flavor, FlavorTranslator, Loc, PackageTranslator, Translate,
           Translator};
use genco::java::{imported, optional, Argument, Field, Method, Modifier, BOOLEAN, DOUBLE, FLOAT,
                  INTEGER, LONG, VOID};
use genco::{Cons, Java};
use naming::{self, Naming};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct JavaHttp<'el> {
    pub request: Java<'el>,
    pub response: Java<'el>,
    pub path: RpPathSpec,
    pub method: RpHttpMethod,
}

#[derive(Debug, Clone)]
pub struct JavaEndpoint<'el> {
    pub endpoint: RpEndpoint,
    pub arguments: Vec<Argument<'el>>,
    pub http1: Option<RpEndpointHttp1>,
}

impl<'el> Deref for JavaEndpoint<'el> {
    type Target = RpEndpoint;

    fn deref(&self) -> &Self::Target {
        &self.endpoint
    }
}

/// A single field.
#[derive(Debug, Clone)]
pub struct JavaField<'el> {
    pub field: RpField,
    pub field_accessor: Rc<String>,
    pub spec: Field<'el>,
}

impl<'el> ::std::ops::Deref for JavaField<'el> {
    type Target = RpField;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<'el> JavaField<'el> {
    pub fn setter(&self) -> Option<Method<'el>> {
        if self.spec.modifiers.contains(&Modifier::Final) {
            return None;
        }

        let argument = Argument::new(self.spec.ty(), self.spec.var());
        let mut m = Method::new(Rc::new(format!("set{}", self.field_accessor)));

        m.arguments.push(argument.clone());

        m.body
            .push(toks!["this.", self.spec.var(), " = ", argument.var(), ";",]);

        Some(m)
    }

    /// Create a new getter method without a body.
    pub fn getter_without_body(&self) -> Method<'el> {
        // Avoid `getClass`, a common built-in method for any Object.
        let field_accessor = match self.field_accessor.as_str() {
            "Class" => "Class_",
            accessor => accessor,
        };

        let mut method = Method::new(Rc::new(format!("get{}", field_accessor)));
        method.comments = self.spec.comments.clone();
        method.returns = self.spec.ty().as_field();
        method
    }

    /// Build a new complete getter.
    pub fn getter(&self) -> Method<'el> {
        let mut m = self.getter_without_body();
        m.body.push(toks!["return this.", self.spec.var(), ";"]);
        m
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JavaFlavor;

impl Flavor for JavaFlavor {
    type Type = Java<'static>;
    type Name = RpName;
    type Field = JavaField<'static>;
    type Endpoint = JavaEndpoint<'static>;
    type Package = core::RpPackage;
}

/// Responsible for translating RpType -> Java type.
pub struct JavaFlavorTranslator {
    package_translator: HashMap<RpVersionedPackage, RpPackage>,
    list: Java<'static>,
    map: Java<'static>,
    string: Java<'static>,
    instant: Java<'static>,
    object: Java<'static>,
    byte_buffer: Java<'static>,
    optional: Java<'static>,
    to_upper_camel: naming::ToUpperCamel,
    to_lower_camel: naming::ToLowerCamel,
}

impl JavaFlavorTranslator {
    pub fn new(package_translator: HashMap<RpVersionedPackage, RpPackage>) -> Self {
        Self {
            package_translator,
            list: imported("java.util", "List"),
            map: imported("java.util", "Map"),
            string: imported("java.lang", "String"),
            instant: imported("java.time", "Instant"),
            object: imported("java.lang", "Object"),
            byte_buffer: imported("java.nio", "ByteBuffer"),
            optional: imported("java.util", "Optional"),
            to_upper_camel: naming::to_upper_camel(),
            to_lower_camel: naming::to_lower_camel(),
        }
    }
}

impl FlavorTranslator for JavaFlavorTranslator {
    type Source = CoreFlavor;
    type Target = JavaFlavor;

    translator_defaults!(Self, local_name);

    fn translate_i32(&self) -> Result<Java<'static>> {
        Ok(INTEGER.into())
    }

    fn translate_i64(&self) -> Result<Java<'static>> {
        Ok(LONG.into())
    }

    fn translate_u32(&self) -> Result<Java<'static>> {
        Ok(INTEGER.into())
    }

    fn translate_u64(&self) -> Result<Java<'static>> {
        Ok(LONG.into())
    }

    fn translate_float(&self) -> Result<Java<'static>> {
        Ok(FLOAT.into())
    }

    fn translate_double(&self) -> Result<Java<'static>> {
        Ok(DOUBLE.into())
    }

    fn translate_boolean(&self) -> Result<Java<'static>> {
        Ok(BOOLEAN.into())
    }

    fn translate_string(&self) -> Result<Java<'static>> {
        Ok(self.string.clone().into())
    }

    fn translate_datetime(&self) -> Result<Java<'static>> {
        Ok(self.instant.clone().into())
    }

    fn translate_array(&self, argument: Java<'static>) -> Result<Java<'static>> {
        Ok(self.list.with_arguments(vec![argument]))
    }

    fn translate_map(&self, key: Java<'static>, value: Java<'static>) -> Result<Java<'static>> {
        Ok(self.map.with_arguments(vec![key, value]))
    }

    fn translate_any(&self) -> Result<Java<'static>> {
        Ok(self.object.clone())
    }

    fn translate_bytes(&self) -> Result<Java<'static>> {
        Ok(self.byte_buffer.clone())
    }

    fn translate_name(&self, reg: RpReg, name: RpName) -> Result<Java<'static>> {
        let ident = Rc::new(reg.ident(&name, |p| p.join("."), |c| c.join(".")));
        let package = name.package.join(".");
        Ok(imported(package, ident))
    }

    fn translate_field<T>(
        &self,
        translator: &T,
        field: core::RpField<CoreFlavor>,
    ) -> Result<JavaField<'static>>
    where
        T: Translator<Source = CoreFlavor, Target = JavaFlavor>,
    {
        let mut field = field.translate(translator)?;

        let field_accessor = Rc::new(self.to_upper_camel.convert(field.ident()));

        let java_type = if field.is_optional() {
            optional(
                field.ty.clone(),
                self.optional.with_arguments(vec![field.ty.clone()]),
            )
        } else {
            field.ty.clone()
        };

        let mut spec = Field::new(java_type, field.safe_ident().to_string());

        if !field.comment.is_empty() {
            spec.comments.push("<pre>".into());
            spec.comments
                .extend(field.comment.drain(..).map(Cons::from));
            spec.comments.push("</pre>".into());
        }

        Ok(JavaField {
            field,
            field_accessor: field_accessor,
            spec: spec,
        })
    }

    fn translate_endpoint<T>(
        &self,
        translator: &T,
        endpoint: core::RpEndpoint<CoreFlavor>,
    ) -> Result<JavaEndpoint<'static>>
    where
        T: Translator<Source = CoreFlavor, Target = JavaFlavor>,
    {
        let endpoint = endpoint.translate(translator)?;

        let mut arguments = Vec::new();

        for arg in &endpoint.arguments {
            let ty = arg.channel.ty().clone();
            arguments.push(Argument::new(ty, arg.safe_ident().to_string()));
        }

        let http1 = RpEndpointHttp1::from_endpoint(&endpoint);

        return Ok(JavaEndpoint {
            endpoint: endpoint,
            arguments: arguments,
            http1: http1,
        });
    }

    fn translate_package(&self, source: RpVersionedPackage) -> Result<RpPackage> {
        Ok(self.package_translator.translate_package(source)?)
    }
}

decl_flavor!(JavaFlavor, core);
