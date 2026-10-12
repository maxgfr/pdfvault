//! Test helpers: every PDF we emit must pass `qpdf --check`.

use std::process::Command;

pub fn qpdf_check(bytes: &[u8]) {
    qpdf_check_with(bytes, None);
}

pub fn qpdf_check_with(bytes: &[u8], password: Option<&str>) {
    let dir = std::env::temp_dir().join(format!("pdfvault-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{:016x}.pdf", fxhash(bytes)));
    std::fs::write(&path, bytes).unwrap();
    let mut cmd = Command::new("qpdf");
    if let Some(p) = password {
        cmd.arg(format!("--password={p}"));
    }
    let out = cmd
        .arg("--check")
        .arg(&path)
        .output()
        .expect("qpdf must be installed to run the tests");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.code() == Some(0),
        "qpdf --check failed ({:?}):\n{}{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn fxhash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, &x| {
        (h ^ x as u64).wrapping_mul(0x100000001b3)
    })
}
