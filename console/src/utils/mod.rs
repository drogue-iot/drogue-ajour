use serde::Serialize;

pub mod float;
pub mod monaco;
pub mod ui;

pub fn to_yaml<T>(content: &T) -> String
where
    T: Serialize,
{
    let yaml = serde_yaml::to_string(content).unwrap_or_default();
    let p: &[_] = &['-', '\n', '\r'];
    yaml.trim_start_matches(p).into()
}
