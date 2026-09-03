//! Type-directed decoding for raw argv/env tokens and structured leaf values.
//!
//! Config resolution mutates `T::default()` field-by-field. This module only
//! decodes one leaf at a time; it never serializes or deserializes the whole
//! config struct.

use crate::spec::Parser;
use crate::{Error, Result};
use serde_core::de::value::{SeqDeserializer, StringDeserializer};
use serde_core::de::{
    self, DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess, IntoDeserializer,
    VariantAccess, Visitor,
};
use serde_json::Value;
use std::fmt;

/// Parse one raw argv/env token into the destination field type.
#[doc(hidden)]
pub fn parse_config_raw<T>(
    field: &str,
    text: &str,
    parser: Parser,
    source: &'static str,
) -> Result<T>
where
    T: DeserializeOwned,
{
    T::deserialize(RawToken {
        text: text.to_string(),
        parser,
        source,
    })
    .map_err(|err| Error::InvalidValue {
        field: field.to_string(),
        message: err.to_string(),
    })
}

/// Parse one already-structured config value into the destination field type.
#[doc(hidden)]
pub fn parse_config_value<T>(field: &str, value: Value) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).map_err(|err| Error::InvalidValue {
        field: field.to_string(),
        message: err.to_string(),
    })
}

#[derive(Debug)]
struct RawToken {
    text: String,
    parser: Parser,
    source: &'static str,
}

#[derive(Debug)]
struct DeError(String);

impl DeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DeError {}

impl de::Error for DeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::new(msg.to_string())
    }
}

fn json_err(err: serde_json::Error) -> DeError {
    DeError::new(err.to_string())
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

    fn structured(&self) -> std::result::Result<Value, DeError> {
        match self.parser {
            Parser::Auto => infer_auto(&self.text).map_err(|message| self.fail(message)),
            Parser::Csv => Ok(Value::Array(
                split_csv(&self.text)
                    .map(|part| Value::String(part.to_string()))
                    .collect(),
            )),
            Parser::Yaml => self.parse_yaml(),
        }
    }

    fn parse_yaml(&self) -> std::result::Result<Value, DeError> {
        #[cfg(feature = "yaml")]
        {
            return yaml_serde::from_str(&self.text)
                .map_err(|err| self.fail(format!("invalid YAML: {err}")));
        }

        #[cfg(not(feature = "yaml"))]
        {
            Err(self.fail("YAML parsing requires Cargo feature `yaml`"))
        }
    }

    fn parse_bool(&self) -> std::result::Result<bool, DeError> {
        let text = self.trimmed();
        if text.eq_ignore_ascii_case("true") || text == "1" {
            return Ok(true);
        }
        if text.eq_ignore_ascii_case("false") || text == "0" {
            return Ok(false);
        }
        Err(self.fail("expected a boolean (true/false or 1/0)"))
    }

    fn parse_number<T>(&self, what: &str) -> std::result::Result<T, DeError>
    where
        T: std::str::FromStr,
    {
        self.trimmed()
            .parse::<T>()
            .map_err(|_| self.fail(format!("expected {what}")))
    }

    fn delegate<'de, V, F>(&self, f: F) -> std::result::Result<V::Value, DeError>
    where
        V: Visitor<'de>,
        F: FnOnce(Value) -> std::result::Result<V::Value, serde_json::Error>,
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
            fn $method<V: Visitor<'de>>(
                self,
                visitor: V,
            ) -> std::result::Result<V::Value, DeError> {
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

    fn deserialize_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        self.delegate::<V, _>(|value| value.deserialize_any(visitor))
    }

    fn deserialize_bool<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
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

    fn deserialize_char<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        if self.parser == Parser::Yaml {
            return self.delegate::<V, _>(|value| value.deserialize_char(visitor));
        }
        let mut chars = self.text.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) => visitor.visit_char(ch),
            _ => Err(self.fail("expected a single character")),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        if self.parser != Parser::Yaml {
            return visitor.visit_string(self.text);
        }

        // Parse first so malformed YAML never degrades into a plain string.
        match self.structured()? {
            Value::String(text) => visitor.visit_string(text),
            Value::Null | Value::Bool(_) | Value::Number(_) => visitor.visit_string(self.text),
            Value::Array(_) | Value::Object(_) => {
                Err(self.fail("expected a YAML scalar for a string field"))
            }
        }
    }

    fn deserialize_bytes<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        visitor.visit_bytes(self.text.as_bytes())
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        visitor.visit_byte_buf(self.text.into_bytes())
    }

    fn deserialize_option<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        if self.is_null() {
            return visitor.visit_none();
        }
        if self.parser == Parser::Yaml {
            let parsed = self.structured()?;
            if parsed.is_null() {
                return visitor.visit_none();
            }
        }
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        if self.is_null() {
            return visitor.visit_unit();
        }
        if self.parser == Parser::Yaml && self.structured()?.is_null() {
            return visitor.visit_unit();
        }
        Err(self.fail("expected null"))
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
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
    ) -> std::result::Result<V::Value, DeError> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
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
    ) -> std::result::Result<V::Value, DeError> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        if self.parser == Parser::Yaml {
            return match self.structured()? {
                Value::String(name) => visitor.visit_enum(UnitVariant { name }),
                value => value.deserialize_enum(name, variants, visitor).map_err(json_err),
            };
        }

        let trimmed = self.trimmed();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return self.delegate::<V, _>(|value| value.deserialize_enum(name, variants, visitor));
        }
        visitor.visit_enum(UnitVariant {
            name: trimmed.to_string(),
        })
    }

    fn deserialize_identifier<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        visitor.visit_unit()
    }
}

struct UnitVariant {
    name: String,
}

impl<'de> EnumAccess<'de> for UnitVariant {
    type Error = DeError;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> std::result::Result<(V::Value, Self), DeError> {
        let deserializer: StringDeserializer<DeError> = self.name.clone().into_deserializer();
        let value = seed.deserialize(deserializer)?;
        Ok((value, self))
    }
}

impl<'de> VariantAccess<'de> for UnitVariant {
    type Error = DeError;

    fn unit_variant(self) -> std::result::Result<(), DeError> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        _seed: T,
    ) -> std::result::Result<T::Value, DeError> {
        Err(DeError::new(format!(
            "variant {:?} given as a bare string can only be a unit variant",
            self.name
        )))
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        _visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        Err(DeError::new(format!(
            "variant {:?} given as a bare string can only be a unit variant",
            self.name
        )))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> std::result::Result<V::Value, DeError> {
        Err(DeError::new(format!(
            "variant {:?} given as a bare string can only be a unit variant",
            self.name
        )))
    }
}

/// Split CSV while preserving explicit empty components.
pub(crate) fn split_csv(text: &str) -> impl Iterator<Item = &str> {
    text.split(',').map(str::trim)
}

/// `auto` inference used for untyped destinations such as `serde_json::Value`.
fn infer_auto(text: &str) -> std::result::Result<Value, String> {
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

    fn parse<T: DeserializeOwned>(text: &str, parser: Parser) -> Result<T> {
        parse_config_raw("value", text, parser, "test")
    }

    #[test]
    fn strings_keep_auto_spelling() {
        assert_eq!(parse::<String>("123", Parser::Auto).unwrap(), "123");
        assert_eq!(parse::<String>("true", Parser::Auto).unwrap(), "true");
        assert_eq!(parse::<String>("null", Parser::Auto).unwrap(), "null");
        assert_eq!(parse::<String>("[1,2]", Parser::Auto).unwrap(), "[1,2]");
    }

    #[test]
    fn bools_are_deliberately_strict() {
        assert!(parse::<bool>("true", Parser::Auto).unwrap());
        assert!(parse::<bool>("1", Parser::Auto).unwrap());
        assert!(!parse::<bool>("false", Parser::Auto).unwrap());
        assert!(!parse::<bool>("0", Parser::Auto).unwrap());
        assert!(parse::<bool>("yes", Parser::Auto).is_err());
        assert!(parse::<bool>("off", Parser::Auto).is_err());
    }

    #[test]
    fn csv_preserves_explicit_empty_components() {
        assert_eq!(
            parse::<Vec<String>>("a,, b,", Parser::Csv).unwrap(),
            vec!["a", "", "b", ""]
        );
        assert_eq!(parse::<Vec<String>>("", Parser::Csv).unwrap(), vec![""]);
        assert!(parse::<Vec<i64>>("1,,2", Parser::Csv).is_err());
    }

    #[test]
    fn csv_is_element_type_directed() {
        assert_eq!(parse::<Vec<i64>>("1, 2,3", Parser::Csv).unwrap(), vec![1, 2, 3]);
        assert_eq!(
            parse::<Vec<String>>("1, 2", Parser::Csv).unwrap(),
            vec!["1".to_string(), "2".to_string()]
        );
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn malformed_yaml_never_falls_back_to_string() {
        assert!(parse::<String>("[broken", Parser::Yaml).is_err());
        assert_eq!(parse::<String>("'hello'", Parser::Yaml).unwrap(), "hello");
    }

    #[cfg(not(feature = "yaml"))]
    #[test]
    fn yaml_parser_reports_the_missing_feature() {
        let err = parse::<String>("hello", Parser::Yaml).unwrap_err();
        assert!(err.to_string().contains("Cargo feature `yaml`"));
    }

    #[test]
    fn options_and_unit_enums_work() {
        assert_eq!(parse::<Option<String>>("none", Parser::Auto).unwrap(), None);

        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Mode {
            Fast,
            Safe,
        }
        assert_eq!(parse::<Mode>("safe", Parser::Auto).unwrap(), Mode::Safe);
        #[cfg(feature = "yaml")]
        {
            assert_eq!(parse::<Mode>("safe", Parser::Yaml).unwrap(), Mode::Safe);
            assert!(parse::<Mode>("[broken", Parser::Yaml).is_err());
        }
    }
}
