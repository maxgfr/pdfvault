//! wasm-bindgen façade over [`pdfvault_render::run`]; same contract as the core module.

use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

fn js_error(e: pdfvault_core::Error) -> JsError {
    let msg = js_sys::JSON::stringify(&JsValue::from_str(&e.to_string()))
        .ok()
        .and_then(|s| s.as_string())
        .unwrap_or_default();
    JsError::new(&format!("{{\"code\":\"{}\",\"message\":{msg}}}", e.code()))
}

#[wasm_bindgen]
pub fn run(op: &str, inputs: Array, options: &str) -> Result<JsValue, JsError> {
    let inputs: Vec<Vec<u8>> = inputs
        .iter()
        .map(|v| Uint8Array::new(&v).to_vec())
        .collect();
    let out = pdfvault_render::run(op, inputs, options).map_err(js_error)?;
    let build = || -> Result<JsValue, JsValue> {
        let files = Array::new();
        for f in out.files {
            let o = Object::new();
            Reflect::set(&o, &"suffix".into(), &f.suffix.into())?;
            Reflect::set(&o, &"ext".into(), &f.ext.into())?;
            Reflect::set(&o, &"bytes".into(), &Uint8Array::from(f.bytes.as_slice()))?;
            files.push(&o);
        }
        let result = Object::new();
        Reflect::set(&result, &"files".into(), &files)?;
        Reflect::set(&result, &"report".into(), &out.report.to_string().into())?;
        Ok(result.into())
    };
    build().map_err(|_| {
        JsError::new("{\"code\":\"internal\",\"message\":\"could not build the result\"}")
    })
}

#[wasm_bindgen]
pub fn ops() -> Vec<String> {
    pdfvault_render::OPS.iter().map(|s| s.to_string()).collect()
}
