//! Deserializer for matchit path parameters. A multi-segment path maps onto a
//! struct or tuple; a single-segment path can also deserialize straight into a
//! newtype or a bare value like `u64` or `String`.

use serde::de;

type Error = de::value::Error;

pub(crate) struct PathDeserializer<'de> {
    params: Vec<(&'de str, &'de str)>,
}

impl<'de> PathDeserializer<'de> {
    pub(crate) fn new(params: &'de matchit::Params<'_, '_>) -> Self {
        Self {
            params: params.iter().collect(),
        }
    }
}

/// Forwards single-value deserialize methods to the lone path parameter, so
/// types like `Path<u64>` work for single-segment paths.
macro_rules! forward_single {
    ($($method:ident)*) => {
        $(
            fn $method<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
                match self.params.as_slice() {
                    [(_, value)] => ValueDeserializer(value).$method(visitor),
                    _ => Err(de::Error::custom("expected a single path parameter")),
                }
            }
        )*
    };
}

impl<'de> de::Deserializer<'de> for PathDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_map<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_map(de::value::MapDeserializer::new(
            self.params
                .into_iter()
                .map(|(k, v)| (k, ValueDeserializer(v))),
        ))
    }

    fn deserialize_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_seq<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_seq(de::value::SeqDeserializer::new(
            self.params.into_iter().map(|(_, v)| ValueDeserializer(v)),
        ))
    }

    fn deserialize_tuple<V: de::Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_newtype_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        // A newtype wrapping a single path parameter, e.g. `struct Id(u64)`,
        // takes its value from that one parameter rather than the param map.
        if let [(_, value)] = self.params.as_slice() {
            visitor.visit_newtype_struct(ValueDeserializer(value))
        } else {
            visitor.visit_newtype_struct(self)
        }
    }

    fn deserialize_unit<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }

    // A single-segment path (e.g. `Path<u64>` or `Path<String>`) deserializes
    // straight from that one value.
    forward_single! {
        deserialize_bool deserialize_char
        deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64 deserialize_i128
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_u128
        deserialize_f32 deserialize_f64
        deserialize_str deserialize_string
    }

    serde::forward_to_deserialize_any! {
        bytes byte_buf option enum identifier ignored_any
    }
}

struct ValueDeserializer<'de>(&'de str);

impl<'de> de::IntoDeserializer<'de, Error> for ValueDeserializer<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

macro_rules! parse_value {
    ($method:ident, $visit:ident, $ty:ty) => {
        fn $method<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            match self.0.parse::<$ty>() {
                Ok(v) => visitor.$visit(v),
                Err(_) => Err(de::Error::custom(concat!("expected ", stringify!($ty)))),
            }
        }
    };
}

impl<'de> de::Deserializer<'de> for ValueDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_borrowed_str(self.0)
    }

    fn deserialize_option<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_enum(de::value::BorrowedStrDeserializer::new(self.0))
    }

    parse_value!(deserialize_bool, visit_bool, bool);
    parse_value!(deserialize_i8, visit_i8, i8);
    parse_value!(deserialize_i16, visit_i16, i16);
    parse_value!(deserialize_i32, visit_i32, i32);
    parse_value!(deserialize_i64, visit_i64, i64);
    parse_value!(deserialize_i128, visit_i128, i128);
    parse_value!(deserialize_u8, visit_u8, u8);
    parse_value!(deserialize_u16, visit_u16, u16);
    parse_value!(deserialize_u32, visit_u32, u32);
    parse_value!(deserialize_u64, visit_u64, u64);
    parse_value!(deserialize_u128, visit_u128, u128);
    parse_value!(deserialize_f32, visit_f32, f32);
    parse_value!(deserialize_f64, visit_f64, f64);

    serde::forward_to_deserialize_any! {
        char str string bytes byte_buf unit unit_struct
        seq tuple tuple_struct map struct identifier ignored_any
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn struct_params() -> anyhow::Result<()> {
        #[derive(Deserialize, PartialEq, Debug)]
        struct P {
            org: String,
            id: u64,
        }

        let mut router = matchit::Router::new();
        router.insert("/{org}/{id}", ())?;
        let matched = router.at("/acme/42")?;

        let v: P = Deserialize::deserialize(PathDeserializer::new(&matched.params))?;
        let expected = P {
            org: "acme".into(),
            id: 42,
        };

        assert_eq!(v, expected);

        Ok(())
    }

    #[test]
    fn newtype_param() -> anyhow::Result<()> {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Id(u64);

        let mut router = matchit::Router::new();
        router.insert("/{id}", ())?;
        let matched = router.at("/42")?;

        let v: Id = Deserialize::deserialize(PathDeserializer::new(&matched.params))?;
        assert_eq!(v, Id(42));

        Ok(())
    }

    #[test]
    fn bare_u64() -> anyhow::Result<()> {
        let mut router = matchit::Router::new();
        router.insert("/{id}", ())?;
        let matched = router.at("/42")?;

        let v: u64 = Deserialize::deserialize(PathDeserializer::new(&matched.params))?;
        assert_eq!(v, 42);

        Ok(())
    }

    #[test]
    fn bare_string() -> anyhow::Result<()> {
        let mut router = matchit::Router::new();
        router.insert("/{name}", ())?;
        let matched = router.at("/hello")?;

        let v: String = Deserialize::deserialize(PathDeserializer::new(&matched.params))?;
        assert_eq!(v, "hello");

        Ok(())
    }

    #[test]
    fn tuple_params() -> anyhow::Result<()> {
        let mut router = matchit::Router::new();
        router.insert("/{org}/{id}", ())?;
        let matched = router.at("/acme/42")?;

        let v: (String, u64) = Deserialize::deserialize(PathDeserializer::new(&matched.params))?;
        assert_eq!(v, ("acme".into(), 42));

        Ok(())
    }
}
