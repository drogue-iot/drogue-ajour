use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/chart.js")]
extern "C" {
    #[wasm_bindgen(js_name = "register_plugin")]
    pub fn register_plugin();

    #[wasm_bindgen(js_name = "gauge_chart")]
    pub fn gauge_chart(props: JsValue, is_update: JsValue);
}
