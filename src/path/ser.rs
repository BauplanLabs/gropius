/// A serializer for path params.
use std::collections::BTreeMap;

use serde::ser::{self, Impossible};
use serde::{Serialize, Serializer};

/// Serialized path parameters: named for a struct, positional for a tuple or a
/// bare scalar.
// This is pub because it's used by the macro.
#[derive(Debug)]
pub enum PathParams {
    /// Parameters keyed by struct field name.
    Named(BTreeMap<String, String>),
    /// Parameters in template order, for a tuple or a bare scalar.
    Positional(Vec<String>),
}

impl PathParams {
    /// The segment for a template parameter, looked up by field name for a
    /// struct or by position for a tuple or scalar.
    pub fn get(&self, name: &str, index: usize) -> Option<&str> {
        match self {
            PathParams::Named(fields) => fields.get(name).map(String::as_str),
            PathParams::Positional(values) => values.get(index).map(String::as_str),
        }
    }
}

/// An error serializing a value into path parameters.
// This is pub because it's used by the macro.
#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

fn unsupported(kind: &str) -> Error {
    Error(format!("`{kind}` cannot be used as a path parameter"))
}

/// Reduce a serialized component to a single segment, rejecting nested
/// sequences or structs that can't fit in one path segment.
fn one_segment(params: PathParams) -> Result<String, Error> {
    match params {
        PathParams::Positional(mut values) if values.len() == 1 => Ok(values.remove(0)),
        _ => Err(unsupported("nested path parameter")),
    }
}

pub(crate) struct PathSerializer;

macro_rules! serialize_scalar {
    ($($method:ident($ty:ty))*) => {
        $(
            fn $method(self, v: $ty) -> Result<PathParams, Error> {
                Ok(PathParams::Positional(vec![v.to_string()]))
            }
        )*
    };
}

impl Serializer for PathSerializer {
    type Ok = PathParams;
    type Error = Error;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = SeqSerializer;
    type SerializeTupleStruct = SeqSerializer;
    type SerializeStruct = StructSerializer;
    type SerializeMap = Impossible<PathParams, Error>;
    type SerializeTupleVariant = Impossible<PathParams, Error>;
    type SerializeStructVariant = Impossible<PathParams, Error>;

    serialize_scalar! {
        serialize_bool(bool)
        serialize_i8(i8) serialize_i16(i16) serialize_i32(i32)
        serialize_i64(i64) serialize_i128(i128)
        serialize_u8(u8) serialize_u16(u16) serialize_u32(u32)
        serialize_u64(u64) serialize_u128(u128)
        serialize_f32(f32) serialize_f64(f64)
        serialize_char(char)
    }

    fn serialize_str(self, v: &str) -> Result<PathParams, Error> {
        Ok(PathParams::Positional(vec![v.to_owned()]))
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<PathParams, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<PathParams, Error> {
        value.serialize(self)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<PathParams, Error> {
        Ok(PathParams::Positional(vec![variant.to_owned()]))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer::default())
    }

    fn serialize_tuple(self, _len: usize) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer::default())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer::default())
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<StructSerializer, Error> {
        Ok(StructSerializer::default())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<PathParams, Error> {
        Err(unsupported("byte string"))
    }

    fn serialize_none(self) -> Result<PathParams, Error> {
        Err(unsupported("missing value"))
    }

    fn serialize_unit(self) -> Result<PathParams, Error> {
        Err(unsupported("unit"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<PathParams, Error> {
        Err(unsupported("unit struct"))
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<PathParams, Error> {
        Err(unsupported("enum variant"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(unsupported("enum variant"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(unsupported("map"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(unsupported("enum variant"))
    }
}

#[derive(Default)]
pub(crate) struct SeqSerializer {
    values: Vec<String>,
}

impl SeqSerializer {
    fn push<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.values
            .push(one_segment(value.serialize(PathSerializer)?)?);
        Ok(())
    }

    fn finish(self) -> Result<PathParams, Error> {
        Ok(PathParams::Positional(self.values))
    }
}

impl ser::SerializeSeq for SeqSerializer {
    type Ok = PathParams;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<PathParams, Error> {
        self.finish()
    }
}

impl ser::SerializeTuple for SeqSerializer {
    type Ok = PathParams;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<PathParams, Error> {
        self.finish()
    }
}

impl ser::SerializeTupleStruct for SeqSerializer {
    type Ok = PathParams;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<PathParams, Error> {
        self.finish()
    }
}

#[derive(Default)]
pub(crate) struct StructSerializer {
    fields: BTreeMap<String, String>,
}

impl ser::SerializeStruct for StructSerializer {
    type Ok = PathParams;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        let segment = one_segment(value.serialize(PathSerializer)?)?;
        self.fields.insert(key.to_owned(), segment);
        Ok(())
    }

    fn end(self) -> Result<PathParams, Error> {
        Ok(PathParams::Named(self.fields))
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::PathSerializer;

    #[test]
    fn struct_params() {
        #[derive(Serialize)]
        struct P {
            org: String,
            id: u64,
        }

        let params = P {
            org: "acme".into(),
            id: 42,
        }
        .serialize(PathSerializer)
        .unwrap();

        assert_eq!(params.get("org", 1), Some("acme"));
        assert_eq!(params.get("id", 0), Some("42"));
        assert_eq!(params.get("missing", 0), None);
    }

    #[test]
    fn tuple_params() {
        let params = ("acme".to_owned(), 42u64)
            .serialize(PathSerializer)
            .unwrap();

        assert_eq!(params.get("whatever", 0), Some("acme"));
        assert_eq!(params.get("whatever", 1), Some("42"));
        assert_eq!(params.get("whatever", 2), None);
    }

    #[test]
    fn newtype_and_scalar() {
        #[derive(Serialize)]
        struct Id(u64);

        assert_eq!(
            Id(7).serialize(PathSerializer).unwrap().get("id", 0),
            Some("7")
        );
        assert_eq!(
            42u64.serialize(PathSerializer).unwrap().get("id", 0),
            Some("42")
        );
        assert_eq!(
            "hi".serialize(PathSerializer).unwrap().get("id", 0),
            Some("hi")
        );
    }

    #[test]
    fn nested_component_is_rejected() {
        #[derive(Serialize)]
        struct Inner {
            a: u64,
        }

        #[derive(Serialize)]
        struct Outer {
            inner: Inner,
        }

        assert!(
            Outer {
                inner: Inner { a: 1 }
            }
            .serialize(PathSerializer)
            .is_err()
        );
    }
}
