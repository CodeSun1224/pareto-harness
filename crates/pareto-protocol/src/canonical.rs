use std::cmp::Ordering;

use serde_json::Value;

use crate::{ErrorCode, ValidationError};

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Serializes the RFC 8785-compatible protocol subset to canonical JSON.
///
/// Protocol floats and integers outside the cross-language safe range are rejected.
pub fn canonical_json(value: &Value) -> Result<String, ValidationError> {
    let mut output = String::new();
    write_value(value, &mut output)?;
    Ok(output)
}

/// Returns canonical UTF-8 bytes for a protocol JSON value.
pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, ValidationError> {
    Ok(canonical_json(value)?.into_bytes())
}

fn write_value(value: &Value, output: &mut String) -> Result<(), ValidationError> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::String(value) => {
            output.push_str(&serde_json::to_string(value).map_err(|_| invalid_number())?)
        }
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_value(value, output)?;
            }
            output.push(']');
        }
        Value::Object(values) => {
            let mut fields: Vec<_> = values.iter().collect();
            fields.sort_by(|(left, _), (right, _)| compare_utf16(left, right));
            output.push('{');
            for (index, (name, value)) in fields.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(name).map_err(|_| invalid_number())?);
                output.push(':');
                write_value(value, output)?;
            }
            output.push('}');
        }
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                if value > MAX_SAFE_INTEGER {
                    return Err(invalid_number());
                }
                output.push_str(&value.to_string());
            } else if let Some(value) = number.as_i64() {
                if value.unsigned_abs() > MAX_SAFE_INTEGER {
                    return Err(invalid_number());
                }
                output.push_str(&value.to_string());
            } else {
                return Err(invalid_number());
            }
        }
    }
    Ok(())
}

fn compare_utf16(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

fn invalid_number() -> ValidationError {
    ValidationError::new(
        ErrorCode::InvariantViolation,
        "",
        "canonical_json",
        "floating point or unsafe integer is forbidden",
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::canonical_json;

    #[test]
    fn sorts_names_as_utf16_and_preserves_array_order() {
        let value = json!({"\u{10000}": 1, "\u{e000}": 2, "array": [2, 1]});
        assert_eq!(
            canonical_json(&value).unwrap(),
            "{\"array\":[2,1],\"𐀀\":1,\"\":2}"
        );
    }

    #[test]
    fn rejects_floats_and_unsafe_integers() {
        assert!(canonical_json(&json!(1.5)).is_err());
        assert!(canonical_json(&json!(9_007_199_254_740_992_u64)).is_err());
    }

    #[test]
    fn matches_rfc_8785_property_sorting_vector() {
        // RFC 8785 section 3.2.3: UTF-16 code-unit ordering of the official sample names.
        let value = json!({"€":"Euro Sign","\r":"Carriage Return","דּ":"Hebrew Letter Dalet With Dagesh","1":"One","😀":"Emoji: Grinning Face","\u{80}":"Control","ö":"Latin Small Letter O With Diaeresis"});
        assert_eq!(
            canonical_json(&value).unwrap(),
            "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\":\"Control\",\"ö\":\"Latin Small Letter O With Diaeresis\",\"€\":\"Euro Sign\",\"😀\":\"Emoji: Grinning Face\",\"דּ\":\"Hebrew Letter Dalet With Dagesh\"}"
        );
    }

    #[test]
    fn matches_rfc_8785_literals_and_string_escaping_vector() {
        // RFC 8785 section 3.2.2 sample, restricted to the protocol's non-floating subset.
        let value = json!({
            "literals": [null, true, false],
            "string": "€$\u{000f}\nA'B\"\\\"/"
        });
        assert_eq!(
            canonical_json(&value).unwrap(),
            "{\"literals\":[null,true,false],\"string\":\"€$\\u000f\\nA'B\\\"\\\\\\\"/\"}"
        );
    }
}
