use std::collections::HashMap;

use serde_json::Value;

use crate::source::model::SettingDefinition;

use super::store::DefaultValue;

/// Parses the `settings.json` file shipped in a source package.
///
/// Newer source packages use setting types that rakuyomi's settings UI doesn't
/// understand (multi-select, pages, editable lists, logins, ...). We only expose
/// the subset that the UI can render, but still keep the defaults for *all*
/// settings so that sources see the values they expect.
pub fn parse_settings(json: &str) -> (Vec<SettingDefinition>, HashMap<String, DefaultValue>) {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return (Vec::new(), HashMap::new());
    };

    let mut defaults = HashMap::new();
    collect_defaults(&value, &mut defaults);

    let definitions = value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(parse_ui_definition)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    (definitions, defaults)
}

fn collect_defaults(value: &Value, defaults: &mut HashMap<String, DefaultValue>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_defaults(item, defaults);
            }
        }
        Value::Object(object) => {
            let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
            let key = object.get("key").and_then(Value::as_str);

            if matches!(kind, "group" | "page") {
                if let Some(items) = object.get("items") {
                    collect_defaults(items, defaults);
                }
                return;
            }

            let Some(key) = key else {
                return;
            };

            let default = object.get("default");
            match kind {
                "switch" => {
                    defaults.insert(
                        key.to_string(),
                        DefaultValue::Bool(default.and_then(Value::as_bool).unwrap_or(false)),
                    );
                }
                "select" | "segment" | "picker" => {
                    if let Some(value) = default.and_then(Value::as_str) {
                        defaults.insert(key.to_string(), DefaultValue::String(value.to_string()));
                    } else if let Some(value) = object
                        .get("values")
                        .and_then(Value::as_array)
                        .and_then(|values| values.first())
                        .and_then(Value::as_str)
                    {
                        defaults.insert(key.to_string(), DefaultValue::String(value.to_string()));
                    }
                }
                "multi-select" | "editable-list" => {
                    let values = default
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    defaults.insert(key.to_string(), DefaultValue::StringArray(values));
                }
                "text" => {
                    if let Some(value) = default.and_then(Value::as_str) {
                        defaults.insert(key.to_string(), DefaultValue::String(value.to_string()));
                    }
                }
                "stepper" => {
                    if let Some(value) = default.and_then(Value::as_f64) {
                        if value.fract() == 0.0 && value.abs() <= i32::MAX as f64 {
                            defaults.insert(key.to_string(), DefaultValue::Int(value as i32));
                        } else {
                            defaults.insert(key.to_string(), DefaultValue::Float(value as f32));
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn parse_ui_definition(value: &Value) -> Option<SettingDefinition> {
    let object = value.as_object()?;
    let kind = object.get("type").and_then(Value::as_str)?;

    match kind {
        "group" => {
            let items = object
                .get("items")?
                .as_array()?
                .iter()
                .filter_map(parse_ui_definition)
                .collect::<Vec<_>>();
            if items.is_empty() {
                return None;
            }
            Some(SettingDefinition::Group {
                title: object
                    .get("title")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                items,
                footer: object
                    .get("footer")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            })
        }
        "page" => {
            let items = object
                .get("items")?
                .as_array()?
                .iter()
                .filter_map(parse_ui_definition)
                .collect::<Vec<_>>();
            if items.is_empty() {
                return None;
            }
            Some(SettingDefinition::Group {
                title: object
                    .get("title")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                items,
                footer: None,
            })
        }
        "select" | "segment" | "picker" => {
            let key = object.get("key").and_then(Value::as_str)?.to_string();
            let values = object
                .get("values")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            if values.is_empty() {
                return None;
            }
            let titles = object
                .get("titles")
                .and_then(Value::as_array)
                .map(|titles| {
                    titles
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                });
            let default = object
                .get("default")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| values[0].clone());
            Some(SettingDefinition::Select {
                title: object
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(&key)
                    .to_string(),
                key,
                values,
                titles,
                default,
            })
        }
        "switch" => {
            let key = object.get("key").and_then(Value::as_str)?.to_string();
            Some(SettingDefinition::Switch {
                title: object
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(&key)
                    .to_string(),
                key,
                default: object
                    .get("default")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        }
        "text" => {
            let key = object.get("key").and_then(Value::as_str)?.to_string();
            Some(SettingDefinition::Text {
                placeholder: object
                    .get("placeholder")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                key,
                default: object
                    .get("default")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            })
        }
        "link" => Some(SettingDefinition::Link {
            title: object.get("title").and_then(Value::as_str)?.to_string(),
            url: object.get("url").and_then(Value::as_str)?.to_string(),
        }),
        _ => None,
    }
}
