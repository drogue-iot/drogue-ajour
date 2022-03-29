use crate::utils::to_yaml;
use monaco::api::TextModel;
use wasm_bindgen::JsValue;

/// Convert content to YAML
pub fn to_yaml_model<T>(content: &T) -> Result<TextModel, JsValue>
where
    T: serde::Serialize,
{
    to_model(Some("yaml"), to_yaml(content))
}

/// Convert content to TextModel
pub fn to_model<S>(language: Option<&str>, text: S) -> Result<TextModel, JsValue>
where
    S: AsRef<str>,
{
    TextModel::create(text.as_ref(), language, None)
}
