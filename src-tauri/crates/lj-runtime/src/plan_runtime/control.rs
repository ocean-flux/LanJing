//! runtime-owned typed value、Condition 与 Merge 纯语义。

use std::cmp::Ordering;
use std::sync::Arc;

use lj_capability::IntentInput;
use lj_rule_model::{ConditionPredicate, MergeStrategy, TypedLiteral, canonical_number_cmp};
use serde_json::{Map, Value};

use crate::effect::{EffectInput, EffectOutput, ExtractOutput, QuickJsOutput};

/// scheduler 内部的闭集值载体。
#[derive(Clone)]
pub(super) enum RuntimeValue {
    Intent(IntentInput),
    Effect(Arc<EffectOutput>),
    Json(Arc<Value>),
    Many(Vec<(String, RuntimeValue)>),
    Decision(String),
    LoopBinding(Arc<Value>),
}

impl RuntimeValue {
    pub(super) fn into_effect_input(self) -> Result<EffectInput, &'static str> {
        match self {
            Self::Intent(input) => Ok(EffectInput::Intent(input)),
            Self::Effect(output) => Ok(EffectInput::Output(output)),
            Self::Json(value) | Self::LoopBinding(value) => Ok(EffectInput::Json(value)),
            Self::Many(_) | Self::Decision(_) => Err("控制值不能直接传给 effect"),
        }
    }

    pub(super) fn json(&self) -> Result<Arc<Value>, &'static str> {
        match self {
            Self::Json(value) | Self::LoopBinding(value) => Ok(value.clone()),
            Self::Intent(input) => Ok(Arc::new(intent_json(input))),
            Self::Effect(output) => effect_json(output.as_ref()).map(Arc::new),
            Self::Many(_) | Self::Decision(_) => Err("控制值不是 JSON"),
        }
    }
}

fn intent_json(input: &IntentInput) -> Value {
    match input {
        IntentInput::Opaque(value) => value.clone(),
        IntentInput::Query(value)
        | IntentInput::ItemId(value)
        | IntentInput::UnitId(value)
        | IntentInput::ActionId(value)
        | IntentInput::Page(value) => Value::String(value.clone()),
        IntentInput::None => Value::Null,
    }
}

fn effect_json(output: &EffectOutput) -> Result<Value, &'static str> {
    match output {
        EffectOutput::QuickJs(QuickJsOutput::Json(value)) => Ok(value.clone()),
        EffectOutput::Extract(ExtractOutput { records }) => Ok(Value::Array(records.clone())),
        EffectOutput::QuickJs(QuickJsOutput::Raw(_)) => Err("QuickJS 输出不是 JSON"),
        EffectOutput::QuickJs(QuickJsOutput::Error(_)) => Err("QuickJS 输出为失败"),
        EffectOutput::Http(_) => Err("HTTP 响应不是 JSON"),
        EffectOutput::Failure(_) => Err("effect 输出为失败"),
    }
}

pub(super) fn evaluate_condition(
    predicate: &ConditionPredicate,
    input: &Value,
) -> Result<bool, &'static str> {
    let selected = input.pointer(predicate.pointer());
    match predicate {
        ConditionPredicate::Exists { .. } => Ok(selected.is_some()),
        ConditionPredicate::IsNull { .. } => Ok(selected.is_some_and(Value::is_null)),
        ConditionPredicate::Eq { value, .. } => {
            Ok(selected.is_some_and(|selected| value.equals_json_value(selected)))
        }
        ConditionPredicate::Ne { value, .. } => {
            Ok(selected.is_some_and(|selected| !value.equals_json_value(selected)))
        }
        ConditionPredicate::Lt { value, .. } => Ok(compare_number(selected, value)
            .transpose()?
            .is_some_and(|order| order == Ordering::Less)),
        ConditionPredicate::Lte { value, .. } => Ok(compare_number(selected, value)
            .transpose()?
            .is_some_and(|order| matches!(order, Ordering::Less | Ordering::Equal))),
        ConditionPredicate::Gt { value, .. } => Ok(compare_number(selected, value)
            .transpose()?
            .is_some_and(|order| order == Ordering::Greater)),
        ConditionPredicate::Gte { value, .. } => Ok(compare_number(selected, value)
            .transpose()?
            .is_some_and(|order| matches!(order, Ordering::Greater | Ordering::Equal))),
        ConditionPredicate::Contains { value, .. } => {
            let Some(selected) = selected else {
                return Ok(false);
            };
            match (selected, value) {
                (Value::String(haystack), TypedLiteral::String(needle)) => {
                    Ok(haystack.contains(needle))
                }
                (Value::Array(values), literal) => Ok(values
                    .iter()
                    .any(|candidate| literal.equals_json_value(candidate))),
                _ => Err("Condition contains 输入类型不匹配"),
            }
        }
    }
}

fn compare_number(
    selected: Option<&Value>,
    literal: &TypedLiteral,
) -> Option<Result<Ordering, &'static str>> {
    let selected = selected?;
    let (Value::Number(selected), TypedLiteral::Number(literal)) = (selected, literal) else {
        return Some(Err("Condition 顺序比较只接受 number"));
    };
    Some(Ok(canonical_number_cmp(selected, literal.as_json_number())))
}

pub(super) fn merge_values(
    strategy: MergeStrategy,
    values: RuntimeValue,
) -> Result<RuntimeValue, &'static str> {
    let RuntimeValue::Many(values) = values else {
        return Err("Merge 缺少有序输入集合");
    };
    match strategy {
        MergeStrategy::SingleActive => {
            let mut values = values.into_iter();
            let Some((_, only)) = values.next() else {
                return Err("single_active 必须恰有一个激活输入");
            };
            if values.next().is_some() {
                return Err("single_active 必须恰有一个激活输入");
            }
            Ok(only)
        }
        MergeStrategy::CollectArray => {
            let mut output = Vec::with_capacity(values.len());
            for (_, value) in values {
                output.push(value.json()?.as_ref().clone());
            }
            Ok(RuntimeValue::Json(Arc::new(Value::Array(output))))
        }
        MergeStrategy::ConcatArrays => {
            let mut output = Vec::new();
            for (_, value) in values {
                let json = value.json()?;
                let Value::Array(items) = json.as_ref() else {
                    return Err("concat_arrays 只接受 array 输入");
                };
                output.extend(items.iter().cloned());
            }
            Ok(RuntimeValue::Json(Arc::new(Value::Array(output))))
        }
        MergeStrategy::OverlayObjects => {
            let mut output = Map::new();
            for (_, value) in values {
                let json = value.json()?;
                let Value::Object(fields) = json.as_ref() else {
                    return Err("overlay_objects 只接受 object 输入");
                };
                output.extend(
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
            }
            Ok(RuntimeValue::Json(Arc::new(Value::Object(output))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: &str, value: Value) -> (String, RuntimeValue) {
        (id.to_string(), RuntimeValue::Json(Arc::new(value)))
    }

    fn merged_json(strategy: MergeStrategy, values: Vec<(String, RuntimeValue)>) -> Arc<Value> {
        merge_values(strategy, RuntimeValue::Many(values))
            .expect("Merge fixture should succeed")
            .json()
            .expect("Merge fixture should produce JSON")
    }

    #[test]
    fn merge_strategies_preserve_declared_order_and_closed_types() {
        assert_eq!(
            merged_json(
                MergeStrategy::CollectArray,
                vec![
                    input("first", serde_json::json!(1)),
                    input("second", serde_json::json!(2))
                ],
            )
            .as_ref(),
            &serde_json::json!([1, 2])
        );
        assert_eq!(
            merged_json(
                MergeStrategy::ConcatArrays,
                vec![
                    input("first", serde_json::json!([1, 2])),
                    input("second", serde_json::json!([3])),
                ],
            )
            .as_ref(),
            &serde_json::json!([1, 2, 3])
        );
        assert_eq!(
            merged_json(
                MergeStrategy::OverlayObjects,
                vec![
                    input("first", serde_json::json!({ "keep": 1, "replace": 1 })),
                    input("second", serde_json::json!({ "replace": 2, "last": 3 })),
                ],
            )
            .as_ref(),
            &serde_json::json!({ "keep": 1, "replace": 2, "last": 3 })
        );
        assert_eq!(
            merged_json(
                MergeStrategy::SingleActive,
                vec![input("only", serde_json::json!({ "value": true }))],
            )
            .as_ref(),
            &serde_json::json!({ "value": true })
        );
        assert!(merge_values(MergeStrategy::SingleActive, RuntimeValue::Many(Vec::new())).is_err());
        assert!(
            merge_values(
                MergeStrategy::SingleActive,
                RuntimeValue::Many(vec![
                    input("first", serde_json::json!(1)),
                    input("second", serde_json::json!(2)),
                ]),
            )
            .is_err()
        );
        assert!(
            merge_values(
                MergeStrategy::ConcatArrays,
                RuntimeValue::Many(vec![input("invalid", serde_json::json!({}))]),
            )
            .is_err()
        );
    }
}
