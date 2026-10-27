//! wasm-bindgen façade over [`pdfvault_ocr::Ocr`]. `init` loads the models
//! and the recognizer's config once per instance; `recognize` returns lines
//! of words as JSON.

use pdfvault_ocr::Ocr;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static ENGINE: RefCell<Option<Ocr>> = const { RefCell::new(None) };
}

fn err(code: &str, msg: &str) -> JsError {
    let msg = js_sys::JSON::stringify(&JsValue::from_str(msg))
        .ok()
        .and_then(|s| s.as_string())
        .unwrap_or_default();
    JsError::new(&format!("{{\"code\":\"{code}\",\"message\":{msg}}}"))
}

#[wasm_bindgen]
pub fn init(detection: Vec<u8>, recognition: Vec<u8>, config: &str) -> Result<(), JsError> {
    let ocr = Ocr::new(detection, recognition, config).map_err(|e| err("ocr_model", &e))?;
    ENGINE.with(|e| *e.borrow_mut() = Some(ocr));
    Ok(())
}

/// `pixels` is RGBA, RGB or gray; returns `[{ text, words: [{ text, x, y, w, h }] }]`.
#[wasm_bindgen]
pub fn recognize(pixels: &[u8], width: u32, height: u32) -> Result<String, JsError> {
    ENGINE.with(|e| {
        let guard = e.borrow();
        let ocr = guard
            .as_ref()
            .ok_or_else(|| err("ocr_not_ready", "call init() with the models first"))?;
        let lines = ocr
            .recognize(pixels, width, height)
            .map_err(|m| err("ocr", &m))?;
        Ok(serde_json_lines(&lines))
    })
}

fn serde_json_lines(lines: &[pdfvault_ocr::Line]) -> String {
    let esc = |s: &str| {
        js_sys::JSON::stringify(&JsValue::from_str(s))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default()
    };
    let mut out = String::from("[");
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("{{\"text\":{},\"words\":[", esc(&l.text)));
        for (j, w) in l.words.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"text\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
                esc(&w.text),
                w.x,
                w.y,
                w.w,
                w.h
            ));
        }
        out.push_str("]}");
    }
    out.push(']');
    out
}
