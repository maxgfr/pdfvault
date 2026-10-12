//! Untrusted bytes through the parser and the most-used tools: they may
//! return errors, never panic, hang or blow past the memory limits.
#![no_main]

use libfuzzer_sys::fuzz_target;
use pdfvault_core::{compress, dispatch, info, pdf};

fuzz_target!(|data: &[u8]| {
    if pdf::load(data, None).is_ok() {
        let _ = info::info(data, None);
        let _ = compress::compress(data, &compress::CompressOptions { preset: compress::Preset::Lossless, ..Default::default() });
        let _ = dispatch::run("select", vec![data.to_vec()], r#"{"pages":"1"}"#);
    }
});
