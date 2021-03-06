//! Go flavor.

#![allow(unused)]

use backend::package_processor;
use core::errors::Result;
use core::{
    self, CoreFlavor, Diagnostics, Flavor, FlavorTranslator, Loc, PackageTranslator, RpNumberKind,
    RpNumberType, RpStringType, Translate, Translator,
};
use genco::go::{array, imported, interface, local, map, Go};
use genco::{Cons, Element};
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;
use trans::Packages;
use TYPE_SEP;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoFlavor;

impl Flavor for GoFlavor {
    type Type = Go<'static>;
    type Name = GoName;
    type Field = RpField;
    type Endpoint = RpEndpoint;
    type Package = RpPackage;
    type EnumType = Go<'static>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoName {
    pub name: Rc<String>,
    pub package: RpPackage,
}

impl fmt::Display for GoName {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(self.name.as_str())
    }
}

impl<'el> From<&'el GoName> for Element<'el, Go<'el>> {
    fn from(value: &'el GoName) -> Element<'el, Go<'el>> {
        Element::Literal(value.name.clone().to_string().into())
    }
}

impl package_processor::Name<GoFlavor> for GoName {
    fn package(&self) -> &RpPackage {
        &self.package
    }
}

/// Responsible for translating RpType -> Go type.
pub struct GoFlavorTranslator {
    package_translator: Rc<Packages>,
}

impl GoFlavorTranslator {
    pub fn new(package_translator: Rc<Packages>) -> Self {
        Self { package_translator }
    }
}

impl FlavorTranslator for GoFlavorTranslator {
    type Source = CoreFlavor;
    type Target = GoFlavor;

    translator_defaults!(Self, field, endpoint);

    fn translate_number(&self, number: RpNumberType) -> Result<Go<'static>> {
        match number.kind {
            RpNumberKind::U32 => Ok(local("uint32")),
            RpNumberKind::U64 => Ok(local("uint64")),
            RpNumberKind::I32 => Ok(local("int32")),
            RpNumberKind::I64 => Ok(local("int64")),
        }
    }

    fn translate_float(&self) -> Result<Go<'static>> {
        Ok(local("float32"))
    }

    fn translate_double(&self) -> Result<Go<'static>> {
        Ok(local("float64"))
    }

    fn translate_boolean(&self) -> Result<Go<'static>> {
        Ok(local("bool"))
    }

    fn translate_string(&self, _: RpStringType) -> Result<Go<'static>> {
        Ok(local("string"))
    }

    fn translate_datetime(&self) -> Result<Go<'static>> {
        Ok(local("string"))
    }

    fn translate_array(&self, argument: Go<'static>) -> Result<Go<'static>> {
        Ok(array(argument))
    }

    fn translate_map(&self, key: Go<'static>, value: Go<'static>) -> Result<Go<'static>> {
        Ok(map(key, value))
    }

    fn translate_any(&self) -> Result<Go<'static>> {
        Ok(interface())
    }

    fn translate_bytes(&self) -> Result<Go<'static>> {
        Ok(local("string"))
    }

    fn translate_name(&self, reg: RpReg, name: Loc<RpName>) -> Result<Go<'static>> {
        let ident = reg.ident(&name, |p| p.join(TYPE_SEP), |c| c.join(TYPE_SEP));

        // imported
        if let Some(_) = name.prefix {
            let module = name.package.join(TYPE_SEP);
            let module = format!("../{}", module);

            return Ok(imported(module, ident));
        }

        // same package
        return Ok(local(ident));
    }

    fn translate_local_name<T>(
        &self,
        translator: &T,
        diag: &mut Diagnostics,
        reg: RpReg,
        name: Loc<core::RpName<CoreFlavor>>,
    ) -> Result<GoName>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>,
    {
        let (name, _) = Loc::take_pair(name);

        let ident = reg.ident(&name, |p| p.join(TYPE_SEP), |c| c.join(TYPE_SEP));
        let package = self.translate_package(name.package)?;

        // same package
        return Ok(GoName {
            name: Rc::new(ident),
            package: package,
        });
    }

    fn translate_package(&self, source: RpVersionedPackage) -> Result<RpPackage> {
        self.package_translator.translate_package(source)
    }

    fn translate_enum_type<T>(
        &self,
        translator: &T,
        diag: &mut Diagnostics,
        enum_type: core::RpEnumType,
    ) -> Result<Go<'static>>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>,
    {
        use core::RpEnumType::*;

        match enum_type {
            String(string) => self.translate_string(string),
            Number(number) => self.translate_number(number),
        }
    }
}

decl_flavor!(GoFlavor, core);
