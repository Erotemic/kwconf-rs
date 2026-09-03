//! The merged value tree and its type-directed deserializer.
//!
//! Structured sources (defaults and config files) are stored as
//! `serde_json::Value`. String-only sources (argv and env) are stored as raw
//! tokens and only coerced once Serde reports which type the destination
//! field wants. This keeps `"123"` a string for a `String` field and an
//! integer for a `u32` field.

use crate::spec::Parser;
use serde::de::value::{SeqDeserializer, StringDeserializer};
use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, IntoDeserializer, MapAccess, VariantAccess,
    Visitor,
};
use serde::forward_to_deserialize_any;
use serde_json::{Map, Value};
use std::collections::{btree_map, BTreeMap};
use std::fmt;

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Value(Value),
    Raw(RawToken),
    Object(BTreeMap<String, Node>),
}

impl Node {
    pub(crate) fn from_object(map: Map<String, Value>) -> BTreeMap<String, Node> {
        map.into_iter()
            .map(|(key, value)| (key, Node::Value(value)))
            .collect()
    }
}

/// A string from argv or env, kept verbatim until the destination type is known.
#[derive(Debug, Clone)]
pub(crate) struct RawToken {
    pub text: String,
    pub parser: Parser,
    pub source: &'static str,
}

/// Deserialization error with the dotted path of the field it belongs to.
#[derive(Debug)]
pub(crate) struct DeError {
    pub path: Vec<String>,
    pub message: String,
}

impl DeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            path: Vec::new(),
            message: message.into(),
        }
    }

    fn prefixed(mut self, key: &str) -> Self {
        self.path.insert(0, key.to_string());
        self
    }
}

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.path.join("."), self.message)
        }
    }
}

impl std::error::Error for DeError {}

impl de::Error for DeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DeError::new(msg.to_string())
    }
}

fn json_err(err: serde_json::Error) -> DeError {
    DeError::new(err.to_string())
}

macro_rules! dispatch {
    ($( fn $method:ident ( $( $arg:ident : $ty:ty ),* ) ),* $(,)?) => {
        $(
            fn $method<V: Visitor<'de>>(self, $($arg: $ty,)* visitor: V) -> Result<V::Value, DeError> {
                match self {
                    Node::Value(value) => value.$method($($arg,)* visitor).map_err(json_err),
                    Node::Raw(token) => token.$method($($arg,)* visitor),
                    Node::Object(map) => ObjectNode(map).$method($($arg,)* visitor),
                }
            }
        )*
    };
}

impl<'de> Deserializer<'de> for Node {
    type Error = DeError;

    dispatch! {
        fn deserialize_any(),
        fn deserialize_bool(),
        fn deserialize_i8(),
        fn deserialize_i16(),
        fn deserialize_i32(),
        fn deserialize_i64(),
        fn deserialize_i128(),
        fn deserialize_u8(),
        fn deserialize_u16(),
        fn deserialize_u32(),
        fn deserialize_u64(),
        fn deserialize_u128(),
        fn deserialize_f32(),
        fn deserialize_f64(),
        fn deserialize_char(),
        fn deserialize_str(),
        fn deserialize_string(),
        fn deserialize_bytes(),
        fn deserialize_byte_buf(),
        fn deserialize_option(),
        fn deserialize_unit(),
        fn deserialize_unit_struct(name: &'static str),
        fn deserialize_newtype_struct(name: &'static str),
        fn deserialize_seq(),
        fn deserialize_tuple(len: usize),
        fn deserialize_tuple_struct(name: &'static str, len: usize),
        fn deserialize_map(),
        fn deserialize_struct(name: &'static str, fields: &'static [&'static str]),
        fn deserialize_enum(name: &'static str, variants: &'static [&'static str]),
        fn deserialize_identifier(),
        fn deserialize_ignored_any(),
    }
}

impl<'de> IntoDeserializer<'de, DeError> for Node {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

struct ObjectNode(BTreeMap<String, Node>);

impl<'de> Deserializer<'de> for ObjectNode {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_map(ObjectAccess {
            iter: self.0.into_iter(),
            pending: None,
        })
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct enum
        identifier
    }
}

struct ObjectAccess {
    iter: btree_map::IntoIter<String, Node>,
    pending: Option<(String, Node)>,
}

impl<'de> MapAccess<'de> for ObjectAccess {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, DeError> {
        match self.iter.next() {
            Some((key, node)) => {
                let deserializer: StringDeserializer<DeError> = key.clone().into_deserializer();
                let value = seed.deserialize(deserializer)?;
                self.pending = Some((key, node));
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<S::Value, DeError> {
        let (key, node) = self
            .pending
            .take()
            .expect("next_value_seed called before next_key_seed");
        seed.deserialize(node).map_err(|err| err.prefixed(&key))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

impl RawToken {
    fn trimmed(&self) -> &str {
        self.text.trim()
    }

    fn is_null(&self) -> bool {
        let text = self.trimmed();
        text.eq_ignore_ascii_case("null") || text.eq_ignore_ascii_case("none")
    }

    fn fail(&self, message: impl fmt::Display) -> DeError {
        let parser = match self.parser {
            Parser::Auto => String::new(),
            other => format!(", {} parser", other.name()),
        };
        DeError::new(format!(
            "{message}, got {:?} ({}{parser})",
            self.text, self.source
        ))
    }

    /// The token interpreted structurally, the way `deserialize_any` sees it.
    fn structured(&self) -> Result<Value, DeError> {
        match self.parser {
            Parser::Auto => infer_auto(&self.text).map_err(|message| self.fail(message)),
            Parser::Csv => Ok(Value::Array(
                split_csv(&self.text)
                    .map(|part| Value::String(part.to_string()))
                    .collect(),
            )),
            Parser::Yaml => yaml_serde::from_str(&self.text)
                .map_err(|err| self.fail(format!("invalid YAML: {err}"))),
        }
    }

    fn parse_bool(&self) -> Result<bool, DeError> {
        let text = self.trimmed();
        for (truthy, falsy) in [("true", "false"), ("1", "0"), ("yes", "no"), ("on", "off")] {
            if text.eq_ignore_ascii_case(truthy) {
                return Ok(true);
            }
            if text.eq_ignore_ascii_case(falsy) {
                return Ok(false);
            }
        }
        Err(self.fail("expected a boolean (true/false, yes/no, on/off, 1/0)"))
    }

    fn parse_number<T>(&self, what: &str) -> Result<T, DeError>
    where
        T: std::str::FromStr,
    {
        self.trimmed()
            .parse::<T>()
            .map_err(|_| self.fail(format!("expected {what}")))
    }

    fn delegate<'de, V, F>(&self, f: F) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
        F: FnOnce(Value) -> Result<V::Value, serde_json::Error>,
    {
        f(self.structured()?).map_err(json_err)
    }
}

impl<'de> IntoDeserializer<'de, DeError> for RawToken {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

macro_rules! raw_number {
    ($( fn $method:ident -> $ty:ty as $visit:ident ($what:literal) ),* $(,)?) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
                if self.parser == Parser::Yaml {
                    return self.delegate::<V, _>(|value| value.$method(visitor));
                }
                let value: $ty = self.parse_number($what)?;
                visitor.$visit(value)
            }
        )*
    };
}

impl<'de> Deserializer<'de> for RawToken {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.delegate::<V, _>(|value| value.deserialize_any(visitor))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        if self.parser == Parser::Yaml {
            return self.delegate::<V, _>(|value| value.deserialize_bool(visitor));
        }
        visitor.visit_bool(self.parse_bool()?)
    }

    raw_number! {
        fn deserialize_i8 -> i64 as visit_i64("an integer"),
        fn deserialize_i16 -> i64 as visit_i64("an integer"),
        fn deserialize_i32 -> i64 as visit_i64("an integer"),
        fn deserialize_i64 -> i64 as visit_i64("an integer"),
        fn deserialize_i128 -> i128 as visit_i128("an integer"),
        fn deserialize_u8 -> u64 as visit_u64("an unsigned integer"),
        fn deserialize_u16 -> u64 as visit_u64("an unsigned integer"),
        fn deserialize_u32 -> u64 as visit_u64("an unsigned integer"),
        fn deserialize_u64 -> u64 as visit_u64("an unsigned integer"),
        fn deserialize_u128 -> u128 as visit_u128("an unsigned integer"),
        fn deserialize_f32 -> f64 as visit_f64("a number"),
        fn deserialize_f64 -> f64 as visit_f64("a number"),
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let mut chars = self.text.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) => visitor.visit_char(ch),
            _ => Err(self.fail("expected a single character")),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        if self.parser == Parser::Yaml {
            if let Ok(Value::String(text)) = self.structured() {
                return visitor.visit_string(text);
            }
        }
        visitor.visit_string(self.text)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_bytes(self.text.as_bytes())
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_byte_buf(self.text.into_bytes())
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        if self.is_null() {
            return visitor.visit_none();
        }
        if self.parser == Parser::Yaml && matches!(self.structured(), Ok(Value::Null)) {
            return visitor.visit_none();
        }
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        if self.is_null()
            || (self.parser == Parser::Yaml && matches!(self.structured(), Ok(Value::Null)))
        {
            visitor.visit_unit()
        } else {
            Err(self.fail("expected null"))
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        if self.parser == Parser::Csv {
            let source = self.source;
            let items: Vec<RawToken> = split_csv(&self.text)
                .map(|part| RawToken {
                    text: part.to_string(),
                    parser: Parser::Auto,
                    source,
                })
                .collect();
            return visitor.visit_seq(SeqDeserializer::new(items.into_iter()));
        }
        match self.structured()? {
            value @ Value::Array(_) => value.deserialize_seq(visitor).map_err(json_err),
            _ => Err(self.fail("expected an array")),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.structured()? {
            value @ Value::Object(_) => value.deserialize_map(visitor).map_err(json_err),
            _ => Err(self.fail("expected an object")),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        let trimmed = self.trimmed();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return self.delegate::<V, _>(|value| value.deserialize_enum(name, variants, visitor));
        }
        visitor.visit_enum(UnitVariant {
            name: trimmed.to_string(),
        })
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_unit()
    }
}

struct UnitVariant {
    name: String,
}

impl<'de> EnumAccess<'de> for UnitVariant {
    type Error = DeError;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), DeError> {
        let deserializer: StringDeserializer<DeError> = self.name.clone().into_deserializer();
        let value = seed.deserialize(deserializer)?;
        Ok((value, self))
    }
}

impl<'de> VariantAccess<'de> for UnitVariant {
    type Error = DeError;

    fn unit_variant(self) -> Result<(), DeError> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value, DeError> {
        Err(DeError::new(format!(
            "variant {:?} given as a bare string can only be a unit variant",
            self.name
        )))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, DeError> {
        Err(DeError::new(format!(
            "variant {:?} given as a bare string can only be a unit variant",
            self.name
        )))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, DeError> {
        Err(DeError::new(format!(
            "variant {:?} given as a bare string can only be a unit variant",
            self.name
        )))
    }
}

/// Split a comma-separated string; an empty or blank string has no parts.
pub(crate) fn split_csv(text: &str) -> impl Iterator<Item = &str> {
    let parts: Vec<&str> = if text.trim().is_empty() {
        Vec::new()
    } else {
        text.split(',').map(str::trim).collect()
    };
    parts.into_iter()
}

/// The `auto` inference used when the destination type is unknown.
pub(crate) fn infer_auto(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }
    if trimmed.eq_ignore_ascii_case("null") || trimmed.eq_ignore_ascii_case("none") {
        return Ok(Value::Null);
    }
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        return serde_json::from_str(trimmed).map_err(|err| format!("invalid JSON: {err}"));
    }
    if let Ok(value) = trimmed.parse::<i64>() {
        return Ok(Value::Number(value.into()));
    }
    if let Ok(value) = trimmed.parse::<u64>() {
        return Ok(Value::Number(value.into()));
    }
    if trimmed.contains(['.', 'e', 'E']) {
        if let Some(number) = trimmed
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
        {
            return Ok(Value::Number(number));
        }
    }
    Ok(Value::String(text.to_string()))
}

/// Text used to check a structured value against `choices`.
pub(crate) fn choice_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    fn raw(text: &str, parser: Parser) -> Node {
        Node::Raw(RawToken {
            text: text.to_string(),
            parser,
            source: "test",
        })
    }

    fn de<T: for<'de> Deserialize<'de>>(node: Node) -> Result<T, DeError> {
        T::deserialize(node)
    }

    #[test]
    fn strings_keep_their_spelling() {
        assert_eq!(de::<String>(raw("123", Parser::Auto)).unwrap(), "123");
        assert_eq!(de::<String>(raw("true", Parser::Auto)).unwrap(), "true");
        assert_eq!(de::<String>(raw("null", Parser::Auto)).unwrap(), "null");
        assert_eq!(de::<String>(raw("[1,2]", Parser::Auto)).unwrap(), "[1,2]");
    }

    #[test]
    fn numbers_and_bools_are_type_directed() {
        assert_eq!(de::<u32>(raw("123", Parser::Auto)).unwrap(), 123);
        assert_eq!(de::<f64>(raw("1e3", Parser::Auto)).unwrap(), 1000.0);
        assert!(de::<bool>(raw("1", Parser::Auto)).unwrap());
        assert!(!de::<bool>(raw("no", Parser::Auto)).unwrap());
        assert!(de::<u8>(raw("300", Parser::Auto)).is_err());
        assert!(de::<u8>(raw("abc", Parser::Auto)).is_err());
    }

    #[test]
    fn options_and_sequences() {
        assert_eq!(
            de::<Option<String>>(raw("none", Parser::Auto)).unwrap(),
            None
        );
        assert_eq!(
            de::<Option<String>>(raw("x", Parser::Auto)).unwrap(),
            Some("x".to_string())
        );
        assert_eq!(
            de::<Vec<i64>>(raw("1, 2,3", Parser::Csv)).unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(
            de::<Vec<String>>(raw("1, 2", Parser::Csv)).unwrap(),
            vec!["1".to_string(), "2".to_string()]
        );
        assert_eq!(
            de::<Vec<i64>>(raw("[1,2]", Parser::Auto)).unwrap(),
            vec![1, 2]
        );
        assert_eq!(
            de::<Vec<String>>(raw("", Parser::Csv)).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            de::<Vec<String>>(raw("[a, b]", Parser::Yaml)).unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn any_keeps_inference() {
        assert_eq!(
            de::<Value>(raw("123", Parser::Auto)).unwrap(),
            Value::from(123)
        );
        assert_eq!(
            de::<Value>(raw("a,b", Parser::Csv)).unwrap(),
            serde_json::json!(["a", "b"])
        );
    }

    #[test]
    fn unit_enums_from_bare_strings() {
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Mode {
            Fast,
            Safe,
        }
        assert_eq!(de::<Mode>(raw("safe", Parser::Auto)).unwrap(), Mode::Safe);
        assert!(de::<Mode>(raw("slow", Parser::Auto)).is_err());
    }

    #[test]
    fn errors_carry_the_field_path() {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Inner {
            count: u8,
        }
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Outer {
            inner: Inner,
        }
        let mut inner = BTreeMap::new();
        inner.insert("count".to_string(), raw("x", Parser::Auto));
        let mut outer = BTreeMap::new();
        outer.insert("inner".to_string(), Node::Object(inner));
        let err = de::<Outer>(Node::Object(outer)).unwrap_err();
        assert_eq!(err.path, vec!["inner".to_string(), "count".to_string()]);
    }
}
