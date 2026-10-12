//! wasm-bindgen façade over [`pdfvault_core::dispatch::run`].

use js_sys::{Array, Object, Reflect, Uint8Array};
use pdfvault_core::dispatch::{self, Output};
use wasm_bindgen::prelude::*;

/// Errors cross the boundary as `{"code": ..., "message": ...}` JSON.
pub fn js_error(e: pdfvault_core::Error) -> JsError {
    let body = format!(
        "{{\"code\":\"{}\",\"message\":{}}}",
        e.code(),
        js_sys::JSON::stringify(&JsValue::from_str(&e.to_string()))
            .ok()
            .and_then(|s| s.as_string())
            .unwrap_or_default()
    );
    JsError::new(&body)
}

pub fn inputs_of(inputs: &Array) -> Vec<Vec<u8>> {
    inputs
        .iter()
        .map(|v| Uint8Array::new(&v).to_vec())
        .collect()
}

/// `{ files: [{ suffix, ext, bytes }], report: string }`
pub fn output_to_js(out: Output) -> Result<JsValue, JsValue> {
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
}

#[wasm_bindgen]
pub fn run(op: &str, inputs: Array, options: &str) -> Result<JsValue, JsError> {
    let out = dispatch::run(op, inputs_of(&inputs), options).map_err(js_error)?;
    output_to_js(out).map_err(|_| {
        JsError::new("{\"code\":\"internal\",\"message\":\"could not build the result\"}")
    })
}

#[wasm_bindgen]
pub fn ops() -> Vec<String> {
    dispatch::OPS.iter().map(|s| s.to_string()).collect()
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}
