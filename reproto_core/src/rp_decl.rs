use std::rc::Rc;
use super::*;
use super::errors::*;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all="snake_case")]
pub enum RpDecl {
    Type(Rc<RpTypeBody>),
    Interface(Rc<RpInterfaceBody>),
    Enum(Rc<RpEnumBody>),
    Tuple(Rc<RpTupleBody>),
    Service(Rc<RpServiceBody>),
}

impl RpDecl {
    pub fn name(&self) -> &str {
        match *self {
            RpDecl::Type(ref body) => &body.name,
            RpDecl::Interface(ref body) => &body.name,
            RpDecl::Enum(ref body) => &body.name,
            RpDecl::Tuple(ref body) => &body.name,
            RpDecl::Service(ref body) => &body.name,
        }
    }
}

impl ::std::fmt::Display for RpDecl {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            RpDecl::Type(ref body) => write!(f, "type {}", body.name),
            RpDecl::Interface(ref body) => write!(f, "interface {}", body.name),
            RpDecl::Enum(ref body) => write!(f, "enum {}", body.name),
            RpDecl::Tuple(ref body) => write!(f, "tuple {}", body.name),
            RpDecl::Service(ref body) => write!(f, "service {}", body.name),
        }
    }
}

impl Merge for RpLoc<RpDecl> {
    fn merge(&mut self, source: RpLoc<RpDecl>) -> Result<()> {
        let dest_pos = self.pos.clone();

        match self.inner {
            RpDecl::Type(ref mut body) => {
                if let RpDecl::Type(other) = source.inner {
                    return body.merge(other);
                }
            }
            RpDecl::Enum(ref mut body) => {
                if let RpDecl::Enum(other) = source.inner {
                    if let Some(variant) = other.variants.iter().next() {
                        return Err(ErrorKind::ExtendEnum("cannot extend enum with additional \
                                                       variants"
                                                             .to_owned(),
                                                         variant.pos.clone(),
                                                         dest_pos)
                            .into());
                    }

                    if let Some(field) = other.fields.iter().next() {
                        return Err(ErrorKind::ExtendEnum("cannot extend enum with additional \
                                                          fields"
                                                             .to_owned(),
                                                         field.pos.clone(),
                                                         dest_pos)
                            .into());
                    }


                    return body.merge(other);
                }
            }
            RpDecl::Interface(ref mut body) => {
                if let RpDecl::Interface(other) = source.inner {
                    return body.merge(other);
                }
            }
            RpDecl::Tuple(ref mut body) => {
                if let RpDecl::Tuple(other) = source.inner {
                    return body.merge(other);
                }
            }
            RpDecl::Service(ref mut body) => {
                if let RpDecl::Service(other) = source.inner {
                    return body.merge(other);
                }
            }
        }

        return Err(ErrorKind::DeclMerge(format!("cannot merge with {}", source),
                                        source.pos,
                                        dest_pos)
            .into());
    }
}