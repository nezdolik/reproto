//! Module that adds fasterxml annotations to generated classes.

use backend::errors::*;
use genco::{Cons, IntoTokens, Java, Quoted, Tokens};
use genco::java::{Argument, Class, DOUBLE, FLOAT, Field, INTEGER, LONG, Modifier, SHORT, imported,
                  local};
use listeners::{ClassAdded, EnumAdded, InterfaceAdded, Listeners, TupleAdded};
use std::rc::Rc;

struct SubTypesType<'a, 'el>(&'a Module, Tokens<'el, Java<'el>>);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for SubTypesType<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@", self.0.sub_type.clone(), "(", self.1.join(", "), ")"]
    }
}

struct SubTypes<'a, 'el>(&'a Module, Tokens<'el, Java<'el>>);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for SubTypes<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@", self.0.sub_types.clone(), "({", self.1.join(", "), "})"]
    }
}

struct TypeInfo<'a, 'el>(&'a Module, Tokens<'el, Java<'el>>);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for TypeInfo<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@", self.0.type_info.clone(), "(", self.1.join(", "), ")"]
    }
}

pub struct Module {
    override_: Java<'static>,
    creator: Java<'static>,
    value: Java<'static>,
    property: Java<'static>,
    sub_types: Java<'static>,
    sub_type: Java<'static>,
    type_info: Java<'static>,
    serialize: Java<'static>,
    deserialize: Java<'static>,
    deserializer: Java<'static>,
    serializer: Java<'static>,
    generator: Java<'static>,
    serializer_provider: Java<'static>,
    parser: Java<'static>,
    deserialization_context: Java<'static>,
    type_reference: Java<'static>,
    token: Java<'static>,
    string: Java<'static>,
    io_exception: Java<'static>,
}

impl Module {
    pub fn new() -> Module {
        Module {
            override_: imported("java.lang", "Override"),
            creator: imported("com.fasterxml.jackson.annotation", "JsonCreator"),
            value: imported("com.fasterxml.jackson.annotation", "JsonValue"),
            property: imported("com.fasterxml.jackson.annotation", "JsonProperty"),
            sub_types: imported("com.fasterxml.jackson.annotation", "JsonSubTypes"),
            sub_type: imported("com.fasterxml.jackson.annotation", "JsonSubTypes").path("Type"),
            type_info: imported("com.fasterxml.jackson.annotation", "JsonTypeInfo"),
            serialize: imported("com.fasterxml.jackson.databind.annotation", "JsonSerialize"),
            deserialize: imported(
                "com.fasterxml.jackson.databind.annotation",
                "JsonDeserialize",
            ),
            serializer: imported("com.fasterxml.jackson.databind", "JsonSerializer"),
            deserializer: imported("com.fasterxml.jackson.databind", "JsonDeserializer"),
            generator: imported("com.fasterxml.jackson.core", "JsonGenerator"),
            serializer_provider: imported("com.fasterxml.jackson.databind", "SerializerProvider"),
            parser: imported("com.fasterxml.jackson.core", "JsonParser"),
            deserialization_context: imported(
                "com.fasterxml.jackson.databind",
                "DeserializationContext",
            ),
            type_reference: imported("com.fasterxml.jackson.core.type", "TypeReference"),
            token: imported("com.fasterxml.jackson.core", "JsonToken"),
            string: imported("java.lang", "String"),
            io_exception: imported("java.io", "IOException"),
        }
    }

    /// RpName serialize implementation for tuples.
    fn tuple_serializer<'el>(
        &self,
        name: Cons<'el>,
        fields: &mut [Field<'el>],
    ) -> Result<Class<'el>> {
        let mut class = Class::new("Serializer");
        let ty = local(name.clone());

        class.extends = Some(self.serializer.with_arguments(vec![ty.clone()]));

        let value = Argument::new(ty.clone(), "value");
        let jgen = Argument::new(self.generator.clone(), "jgen");
        let provider = Argument::new(self.serializer_provider.clone(), "provider");

        let mut serialize = Tokens::new();

        serialize.push(toks!("@", self.override_.clone()));
        serialize.push(toks![
            "public serialize(",
            toks![value.var(), jgen.var(), provider.var()].join_spacing(),
            ") throws ",
            self.io_exception.clone(),
            " {",
        ]);

        serialize.nested({
            let mut t = Tokens::new();
            t.push(toks!["jgen.writeStartArray();"]);

            for field in fields {
                let access = toks!["value.", field.var()];

                let write = match field.ty() {
                    SHORT | LONG | INTEGER | FLOAT | DOUBLE => {
                        toks!["writeNumber(", access.clone(), ")"]
                    }
                    Java::Primitive { .. } => {
                        return Err("cannot serialize type".into());
                    }
                    class @ Java::Class { .. } => {
                        if class == self.string {
                            toks!["writeString(", access.clone(), ")"]
                        } else {
                            toks!["writeObject(", access.clone(), ")"]
                        }
                    }
                    _ => toks!["writeObject(", access.clone(), ")"],
                };

                t.push(toks!["jgen.", write, ";"]);
            }

            t.push(toks!["jgen.writeEndArray();"]);

            t
        });

        serialize.push("}");

        class.body.push(serialize);
        Ok(class)
    }

    fn deserialize_method_for_type<'el, A>(
        &self,
        ty: Java<'el>,
        parser: A,
    ) -> Result<(Option<(Tokens<'el, Java<'el>>, &'el str)>, Tokens<'el, Java<'el>>)>
    where
        A: Into<Tokens<'el, Java<'el>>>,
    {
        let p = parser.into();

        let (token, reader) = match ty {
            java @ Java::Primitive { .. } => {
                let test = toks!["!", p.clone(), ".nextToken().isNumeric()"];

                match java {
                    SHORT => (
                        Some((test, "VALUE_NUMBER_INT")),
                        toks![p, ".getShortValue()"],
                    ),
                    LONG => (
                        Some((test, "VALUE_NUMBER_INT")),
                        toks![p, ".getLongValue()"],
                    ),
                    INTEGER => {
                        (
                            Some((test, "VALUE_NUMBER_INT")),
                            toks![p, ".getIntegerValue()"],
                        )
                    }
                    FLOAT => {
                        (
                            Some((test, "VALUE_NUMBER_FLOAT")),
                            toks![p, ".getFloatValue()"],
                        )
                    }
                    DOUBLE => {
                        (
                            Some((test, "VALUE_NUMBER_FLOAT")),
                            toks![p, ".getDoubleValue()"],
                        )
                    }
                    _ => {
                        return Err("unsupported type".into());
                    }
                }
            }
            class @ Java::Class { .. } => {
                if class == self.string {
                    let test =
                        toks![p.clone(), ".nextToken() != ", self.token.clone(), ".VALUE_STRING",
                    ];
                    let token = Some((test, "VALUE_STRING"));
                    (token, toks![p, ".getText()"])
                } else {
                    let is_empty = class.arguments().map(|a| a.is_empty()).unwrap_or(true);

                    let argument = if is_empty {
                        toks![class, ".class"]
                    } else {
                        toks![
                            "new ",
                            self.type_reference.with_arguments(vec![class]),
                            "(){}",
                        ]
                    };

                    (None, toks![p, ".readValueAs(", argument, ")"])
                }
            }
            _ => {
                return Err("unsupported type".into());
            }
        };

        Ok((token, reader))
    }

    fn wrong_token<'el, C, P, T>(&self, ctxt: C, parser: P, token: T) -> Tokens<'el, Java<'el>>
    where
        C: Into<Tokens<'el, Java<'el>>>,
        P: Into<Tokens<'el, Java<'el>>>,
        T: Into<Tokens<'el, Java<'el>>>,
    {
        let mut arguments = Tokens::new();

        arguments.push(parser.into());
        arguments.push(toks![self.token.clone(), ".", token.into()]);
        arguments.push("null");

        toks![
            "throw ",
            ctxt.into(),
            ".wrongTokenException(",
            arguments.join(", "),
            ");",
        ]
    }

    /// RpName deserialize implementation for tuples.
    fn tuple_deserializer<'el>(
        &self,
        name: Cons<'el>,
        fields: &mut [Field<'el>],
    ) -> Result<Class<'el>> {
        use self::Modifier::*;

        let ty = local(name.clone());

        let parser = toks!("final ", self.parser.clone(), " parser");
        let ctxt = toks!("final ", self.deserialization_context.clone(), " ctxt");

        let mut deserialize = Tokens::new();

        deserialize.push(toks!("@", self.override_.clone()));
        deserialize.push(toks![
            "public ", ty.clone(), " deserialize(",
            toks![parser, ctxt].join_spacing(),
            ") throws ",
            self.io_exception.clone(),
            " {",
        ]);

        deserialize.nested({
            let mut body = Tokens::new();
            let current_token = toks!["parser.getCurrentToken()"];

            let mut start_array = Tokens::new();
            start_array.push(toks![
                "if (",
                current_token,
                " != ",
                self.token.clone(),
                ".START_ARRAY) {",
            ]);
            start_array.nested(self.wrong_token("ctxt", "parser", "START_ARRAY"));
            start_array.push("}");
            body.push(start_array);

            let mut arguments = Tokens::new();

            for field in fields {
                let (token, reader) = self.deserialize_method_for_type(field.ty(), "parser")?;

                if let Some((test, expected)) = token {
                    let mut field_check = Tokens::new();
                    field_check.push(toks!["if (", test, ") {"]);
                    field_check.nested(self.wrong_token("ctxt", "parser", expected));
                    field_check.push("}");
                    body.push(field_check);
                }

                let variable = toks!["v_", field.var()];
                let assign =
                    toks![
                        "final ",
                        field.ty(),
                        " ",
                        variable.clone(),
                        " = ",
                        reader,
                        ";",
                    ];
                body.push(assign);
                arguments.push(variable);
            }

            let mut end_array = Tokens::new();
            end_array.push(toks![
                "if (parser.nextToken() != ", self.token.clone(), ".END_ARRAY) {",
            ]);
            end_array.nested(self.wrong_token("ctxt", "parser", "END_ARRAY"));
            end_array.push("}");
            body.push(end_array);

            body.push(toks![
                "return new ",
                ty.clone(),
                "(",
                arguments.join(", "),
                ");",
            ]);

            body.join_line_spacing()
        });

        deserialize.push("}");

        Ok({
            let mut deserializer = Class::new("Deserializer");
            deserializer.modifiers.push(Static);
            deserializer.extends = Some(self.deserializer.with_arguments(vec![ty.clone()]));
            deserializer.body.push(deserialize);
            deserializer
        })
    }

    fn add_class_annotations<'a>(&self, names: &[Cons<'a>], spec: &mut Class<'a>) -> Result<()> {
        for (field, name) in spec.fields.iter_mut().zip(names.iter()) {
            let ann = toks!["@", self.property.clone(), "(", name.clone().quoted(), ")"];
            field.annotation(ann.clone());
        }

        for c in &mut spec.constructors {
            c.annotation(toks!["@", self.creator.clone()]);

            for (argument, name) in c.arguments.iter_mut().zip(names.iter()) {
                let ann = toks!["@", self.property.clone(), "(", name.clone().quoted(), ")"];
                argument.annotation(ann.clone());
            }
        }

        Ok(())
    }

    fn add_tuple_serialization(&self, spec: &mut Class) -> Result<()> {
        let serializer = self.tuple_serializer(spec.name(), &mut spec.fields)?;

        let serializer_type = Rc::new(format!(
            "{}.{}",
            spec.name().as_ref(),
            serializer.name().as_ref()
        ));

        spec.annotation(toks![
            self.serialize.clone(),
            "(using = ",
            serializer_type,
            ".class)",
        ]);

        spec.body.push(serializer);

        let deserializer = self.tuple_deserializer(spec.name(), &mut spec.fields)?;

        let deserializer_type = Rc::new(format!(
            "{}.{}",
            spec.name().as_ref(),
            deserializer.name().as_ref()
        ));

        let deserialize =
            toks![
            "@",
            self.deserialize.clone(),
            "(using = ",
            deserializer_type,
            ".class)",
        ];

        spec.annotation(deserialize);
        spec.body.push(deserializer);
        Ok(())
    }
}

impl Listeners for Module {
    fn class_added<'a>(&self, e: &mut ClassAdded) -> Result<()> {
        self.add_class_annotations(&e.names, &mut e.spec)?;
        Ok(())
    }

    fn tuple_added(&self, e: &mut TupleAdded) -> Result<()> {
        self.add_tuple_serialization(&mut e.spec)
    }

    fn enum_added(&self, e: &mut EnumAdded) -> Result<()> {
        e.from_value.annotation(toks!["@", self.creator.clone()]);
        e.to_value.annotation(toks!["@", self.value.clone()]);
        Ok(())
    }

    fn interface_added(&self, e: &mut InterfaceAdded) -> Result<()> {
        {
            let mut args = Tokens::new();

            args.append(toks!["use=", self.type_info.clone(), ".Id.NAME"]);
            args.append(toks!["include=", self.type_info.clone(), ".As.PROPERTY"]);
            args.append(toks!["property=", "type".quoted()]);

            e.spec.annotation(TypeInfo(self, args));
        }

        {
            let mut args = Tokens::new();

            for (key, sub_type) in &e.body.sub_types {
                for name in &sub_type.names {
                    let name = name.value().to_owned();

                    let mut a = Tokens::new();

                    a.append(toks!["name=", name.quoted()]);
                    a.append(toks![
                        "value=",
                        e.spec.name(),
                        ".",
                        key.as_str(),
                        ".class",
                    ]);

                    args.push(SubTypesType(self, a));
                }
            }

            e.spec.annotation(SubTypes(self, args));
        }

        Ok(())
    }
}
