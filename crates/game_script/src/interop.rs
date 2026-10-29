use std::fmt;
use std::str::FromStr;
use serde_json::{Map, Number, Value};
use steel::gc::Gc;
use steel::rvals::SteelVal;
use steel::HashMap;

/// Errors occurring during bidirectional Scheme <-> JSON data conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteropError {
    UnsupportedType(String),
    InvalidNumber(String),
    Serialization(String),
}

impl fmt::Display for InteropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedType(t) => write!(f, "Unsupported SteelVal type for JSON conversion: {}", t),
            Self::InvalidNumber(n) => write!(f, "Invalid number for JSON: {}", n),
            Self::Serialization(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for InteropError {}

/// Convert a Scheme [`SteelVal`] reference to a [`serde_json::Value`].
///
/// Recursively maps:
/// - `BoolV(b)` -> `Value::Bool(b)`
/// - `NumV(f)` / `IntV(i)` / `Rational(r)` -> `Value::Number`
/// - `StringV(s)` / `SymbolV(s)` / `CharV(c)` -> `Value::String`
/// - `Void` -> `Value::Null`
/// - `ListV(list)` / `VectorV(vec)` -> `Value::Array`
/// - `HashMapV(map)` -> `Value::Object` (keys formatted as strings)
pub fn steel_val_to_json(val: &SteelVal) -> Result<Value, InteropError> {
    match val {
        SteelVal::BoolV(b) => Ok(Value::Bool(*b)),
        SteelVal::NumV(n) => {
            Number::from_f64(*n)
                .map(Value::Number)
                .ok_or_else(|| InteropError::InvalidNumber(format!("Non-finite f64: {}", n)))
        }
        SteelVal::IntV(i) => Ok(Value::Number((*i as i64).into())),
        SteelVal::Rational(r) => {
            let f = (*r.numer() as f64) / (*r.denom() as f64);
            Number::from_f64(f)
                .map(Value::Number)
                .ok_or_else(|| InteropError::InvalidNumber(format!("Rational to f64 failed: {}", r)))
        }
        SteelVal::BigNum(bn) => {
            let s = bn.to_string();
            if let Ok(num) = Number::from_str(&s) {
                Ok(Value::Number(num))
            } else {
                Ok(Value::String(s))
            }
        }
        SteelVal::StringV(s) => Ok(Value::String(s.to_string())),
        SteelVal::SymbolV(s) => Ok(Value::String(s.to_string())),
        SteelVal::CharV(c) => Ok(Value::String(c.to_string())),
        SteelVal::Void => Ok(Value::Null),
        SteelVal::ListV(l) => {
            let mut arr = Vec::with_capacity(l.len());
            for item in l {
                arr.push(steel_val_to_json(item)?);
            }
            Ok(Value::Array(arr))
        }
        SteelVal::VectorV(v) => {
            let mut arr = Vec::with_capacity(v.len());
            for item in v.iter() {
                arr.push(steel_val_to_json(item)?);
            }
            Ok(Value::Array(arr))
        }
        SteelVal::HashMapV(hm) => {
            let mut map = Map::new();
            for (k, v) in hm.iter() {
                let key = match k {
                    SteelVal::StringV(s) => s.to_string(),
                    SteelVal::SymbolV(s) => s.to_string(),
                    SteelVal::IntV(i) => i.to_string(),
                    other => format!("{}", other),
                };
                map.insert(key, steel_val_to_json(v)?);
            }
            Ok(Value::Object(map))
        }
        SteelVal::HashSetV(hs) => {
            let mut arr = Vec::with_capacity(hs.len());
            for item in hs.iter() {
                arr.push(steel_val_to_json(item)?);
            }
            Ok(Value::Array(arr))
        }
        other => Err(InteropError::UnsupportedType(format!("{:?}", other))),
    }
}

/// Convert a [`serde_json::Value`] reference to a Scheme [`SteelVal`].
///
/// Recursively maps:
/// - `Value::Null` -> `SteelVal::Void`
/// - `Value::Bool(b)` -> `SteelVal::BoolV(b)`
/// - `Value::Number(n)` -> `SteelVal::IntV` (if integer) or `SteelVal::NumV` (if float)
/// - `Value::String(s)` -> `SteelVal::StringV(s)`
/// - `Value::Array(arr)` -> `SteelVal::ListV(List<SteelVal>)`
/// - `Value::Object(obj)` -> `SteelVal::HashMapV(HashMap<SteelVal, SteelVal>)`
pub fn json_to_steel_val(val: &Value) -> SteelVal {
    match val {
        Value::Null => SteelVal::Void,
        Value::Bool(b) => SteelVal::BoolV(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SteelVal::IntV(i as isize)
            } else if let Some(u) = n.as_u64() {
                SteelVal::IntV(u as isize)
            } else if let Some(f) = n.as_f64() {
                SteelVal::NumV(f)
            } else {
                SteelVal::Void
            }
        }
        Value::String(s) => SteelVal::StringV(s.as_str().into()),
        Value::Array(arr) => {
            let items: Vec<SteelVal> = arr.iter().map(json_to_steel_val).collect();
            SteelVal::ListV(items.into())
        }
        Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(SteelVal::StringV(k.as_str().into()), json_to_steel_val(v));
            }
            SteelVal::HashMapV(Gc::new(map).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_primitives_roundtrip() {
        // Null / Void
        assert_eq!(steel_val_to_json(&SteelVal::Void).unwrap(), Value::Null);
        assert_eq!(json_to_steel_val(&Value::Null), SteelVal::Void);

        // Bool
        assert_eq!(steel_val_to_json(&SteelVal::BoolV(true)).unwrap(), Value::Bool(true));
        assert_eq!(json_to_steel_val(&Value::Bool(false)), SteelVal::BoolV(false));

        // Int & Float
        assert_eq!(steel_val_to_json(&SteelVal::IntV(42)).unwrap(), json!(42));
        assert_eq!(steel_val_to_json(&SteelVal::NumV(3.14)).unwrap(), json!(3.14));
        assert_eq!(json_to_steel_val(&json!(100)), SteelVal::IntV(100));
        assert_eq!(json_to_steel_val(&json!(2.5)), SteelVal::NumV(2.5));

        // String & Symbol
        assert_eq!(steel_val_to_json(&SteelVal::StringV("hello".into())).unwrap(), json!("hello"));
        assert_eq!(steel_val_to_json(&SteelVal::SymbolV("world".into())).unwrap(), json!("world"));
        assert_eq!(json_to_steel_val(&json!("foo")), SteelVal::StringV("foo".into()));
    }

    #[test]
    fn test_nested_structures() {
        let json_input = json!({
            "name": "iron_sword",
            "damage": 12,
            "durability": 250.5,
            "unbreakable": false,
            "enchantments": ["sharpness", "unbreaking"],
            "meta": {
                "rarity": "rare",
                "custom_model_data": 104
            }
        });

        let steel_val = json_to_steel_val(&json_input);
        let roundtrip_json = steel_val_to_json(&steel_val).unwrap();

        assert_eq!(json_input, roundtrip_json);
    }
}
