use patternfly_yew::*;
use serde_json::Value;
use yew::prelude::*;

pub fn render_payload(data: &[u8], expanded: bool) -> Html {
    if let Ok(json) = serde_json::from_slice::<Value>(data) {
        let json = match expanded {
            true => serde_json::to_string_pretty(&json).unwrap_or_default(),
            false => serde_json::to_string(&json).unwrap_or_default(),
        };
        return html!(
            <code><pre>
                {json}
            </pre></code>
        );
    }

    if let Ok(str) = String::from_utf8(data.to_vec()) {
        return html!(
            <pre>
                {str}
            </pre>
        );
    }

    html!("Binary data")
}

pub trait ToDetail {
    fn to_details(&self) -> (String, String);
}

impl<V> ToDetail for (&str, V)
where
    V: ToString,
{
    fn to_details(&self) -> (String, String) {
        (self.0.into(), self.1.to_string())
    }
}

pub fn details<'d, const N: usize>(details: [&dyn ToDetail; N]) -> Html {
    html!(
        <Form>
          { for details.into_iter().map(|details|{
              let (label, value) = details.to_details();
              html!(
                  <FormGroup
                    label={format!("{label}:")}
                  >
                    <TextInput value={value} readonly=true />
                  </FormGroup>
              )
          })}
        </Form>
    )
}
