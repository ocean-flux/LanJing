//! 控制表达式使用的 typed JSON literal 与数值语义。
//!
//! JSON number 比较按十进制数值而不是 `serde_json::Number` 的内部表示；对象键仍按 JSON
//! 名称匹配，数组保持顺序。

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize, Serializer};

/// 以规范十进制 wire 形状序列化、并按数值实现相等性的 JSON number。
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct CanonicalNumber(serde_json::Number);

impl CanonicalNumber {
    /// 包装一个合法 JSON number。
    #[must_use]
    pub const fn new(value: serde_json::Number) -> Self {
        Self(value)
    }

    /// 借用原始 JSON number，供 runtime 与 JSON Pointer 结果比较。
    #[must_use]
    pub const fn as_json_number(&self) -> &serde_json::Number {
        &self.0
    }

    /// 返回原始 JSON number。
    #[must_use]
    pub fn into_json_number(self) -> serde_json::Number {
        self.0
    }
}

impl PartialEq for CanonicalNumber {
    fn eq(&self, other: &Self) -> bool {
        canonical_number_eq(&self.0, &other.0)
    }
}

impl Eq for CanonicalNumber {}

impl Serialize for CanonicalNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NormalizedNumber::from_json(&self.0)
            .to_json_number()
            .serialize(serializer)
    }
}

/// Condition 配置可持久化的闭集 JSON literal。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypedLiteral {
    /// JSON null。
    Null,
    /// JSON boolean。
    Bool(bool),
    /// 按数值比较的 JSON number。
    Number(CanonicalNumber),
    /// JSON string。
    String(String),
    /// 有序 JSON array。
    Array(Vec<TypedLiteral>),
    /// 按键匹配的 JSON object。
    Object(BTreeMap<String, TypedLiteral>),
}

impl TypedLiteral {
    /// 判断 literal 是否与 runtime JSON 值按 canonical deep equality 相等。
    #[must_use]
    pub fn equals_json_value(&self, value: &serde_json::Value) -> bool {
        typed_literal_matches_json(self, value)
    }
}

/// 按 JSON 十进制数值比较两个 number。
///
/// `1`、`1.0` 与 `1e0` 相等；负零与零相等。比较不经过 `f64`，因此不会丢失整数精度。
#[must_use]
pub fn canonical_number_cmp(left: &serde_json::Number, right: &serde_json::Number) -> Ordering {
    NormalizedNumber::from_json(left).cmp(&NormalizedNumber::from_json(right))
}

/// 判断两个 JSON number 是否具有相同十进制数值。
#[must_use]
pub fn canonical_number_eq(left: &serde_json::Number, right: &serde_json::Number) -> bool {
    canonical_number_cmp(left, right) == Ordering::Equal
}

/// 按 canonical JSON deep equality 比较两个 runtime JSON 值。
///
/// object 键集合与 array 顺序必须相同；number 使用 [`canonical_number_eq`]。
#[must_use]
pub fn canonical_json_deep_eq(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    match (left, right) {
        (serde_json::Value::Null, serde_json::Value::Null) => true,
        (serde_json::Value::Bool(left), serde_json::Value::Bool(right)) => left == right,
        (serde_json::Value::Number(left), serde_json::Value::Number(right)) => {
            canonical_number_eq(left, right)
        }
        (serde_json::Value::String(left), serde_json::Value::String(right)) => left == right,
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| canonical_json_deep_eq(left, right))
        }
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| canonical_json_deep_eq(left, right))
                })
        }
        _ => false,
    }
}

/// 判断 typed literal 是否与 runtime JSON 值按 canonical deep equality 相等。
#[must_use]
pub fn typed_literal_matches_json(literal: &TypedLiteral, value: &serde_json::Value) -> bool {
    match (literal, value) {
        (TypedLiteral::Null, serde_json::Value::Null) => true,
        (TypedLiteral::Bool(left), serde_json::Value::Bool(right)) => left == right,
        (TypedLiteral::Number(left), serde_json::Value::Number(right)) => {
            canonical_number_eq(left.as_json_number(), right)
        }
        (TypedLiteral::String(left), serde_json::Value::String(right)) => left == right,
        (TypedLiteral::Array(left), serde_json::Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| typed_literal_matches_json(left, right))
        }
        (TypedLiteral::Object(left), serde_json::Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| typed_literal_matches_json(left, right))
                })
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedNumber {
    negative: bool,
    digits: String,
    exponent: i32,
}

impl NormalizedNumber {
    fn from_json(value: &serde_json::Number) -> Self {
        let source = value.to_string();
        let (negative, unsigned) = source
            .strip_prefix('-')
            .map_or((false, source.as_str()), |value| (true, value));
        let (mantissa, exponent) =
            unsigned
                .split_once(['e', 'E'])
                .map_or((unsigned, 0), |(mantissa, exponent)| {
                    (
                        mantissa,
                        exponent
                            .parse::<i32>()
                            .expect("serde_json::Number exponent must be valid"),
                    )
                });
        let (integer, fraction) = mantissa
            .split_once('.')
            .map_or((mantissa, ""), |(integer, fraction)| (integer, fraction));
        let mut digits = String::with_capacity(integer.len() + fraction.len());
        digits.push_str(integer);
        digits.push_str(fraction);
        let first_non_zero = digits.find(|character| character != '0');
        let Some(first_non_zero) = first_non_zero else {
            return Self {
                negative: false,
                digits: "0".to_string(),
                exponent: 0,
            };
        };
        digits.drain(..first_non_zero);
        let mut exponent = exponent
            - i32::try_from(fraction.len()).expect("JSON number fraction length must fit i32");
        while digits.ends_with('0') {
            digits.pop();
            exponent += 1;
        }
        Self {
            negative,
            digits,
            exponent,
        }
    }

    fn to_json_number(&self) -> serde_json::Number {
        let mut canonical = String::new();
        if self.negative {
            canonical.push('-');
        }
        if self.digits == "0" {
            canonical.push('0');
            return serde_json::Number::from_str(&canonical)
                .expect("canonical zero must be a JSON number");
        }
        let decimal_position = i64::try_from(self.digits.len()).expect("digit length must fit i64")
            + i64::from(self.exponent);
        if decimal_position <= 0 {
            canonical.push_str("0.");
            canonical.extend(std::iter::repeat_n(
                '0',
                usize::try_from(-decimal_position).expect("JSON decimal position must fit usize"),
            ));
            canonical.push_str(&self.digits);
        } else if decimal_position
            >= i64::try_from(self.digits.len()).expect("digit length must fit i64")
        {
            canonical.push_str(&self.digits);
            canonical.extend(std::iter::repeat_n(
                '0',
                usize::try_from(
                    decimal_position
                        - i64::try_from(self.digits.len()).expect("digit length must fit i64"),
                )
                .expect("JSON decimal padding must fit usize"),
            ));
        } else {
            let position = usize::try_from(decimal_position)
                .expect("positive JSON decimal position must fit usize");
            canonical.push_str(&self.digits[..position]);
            canonical.push('.');
            canonical.push_str(&self.digits[position..]);
        }
        serde_json::Number::from_str(&canonical).expect("normalized decimal must be a JSON number")
    }

    fn magnitude_cmp(&self, other: &Self) -> Ordering {
        let left_magnitude = i64::try_from(self.digits.len()).expect("digit length must fit i64")
            + i64::from(self.exponent);
        let right_magnitude = i64::try_from(other.digits.len()).expect("digit length must fit i64")
            + i64::from(other.exponent);
        match left_magnitude.cmp(&right_magnitude) {
            Ordering::Equal => {
                let width = self.digits.len().max(other.digits.len());
                for index in 0..width {
                    let left = self.digits.as_bytes().get(index).copied().unwrap_or(b'0');
                    let right = other.digits.as_bytes().get(index).copied().unwrap_or(b'0');
                    match left.cmp(&right) {
                        Ordering::Equal => {}
                        ordering => return ordering,
                    }
                }
                Ordering::Equal
            }
            ordering => ordering,
        }
    }
}

impl Ord for NormalizedNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.digits == "0" && other.digits == "0" {
            return Ordering::Equal;
        }
        match self.negative.cmp(&other.negative) {
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
            Ordering::Equal if self.negative => self.magnitude_cmp(other).reverse(),
            Ordering::Equal => self.magnitude_cmp(other),
        }
    }
}

impl PartialOrd for NormalizedNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
