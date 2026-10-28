//! `cargo run -p pdfvault-fixtures -- <dir>`: write every generated fixture.

use pdfvault_core::fixtures as f;
use pdfvault_core::security::{EncryptOptions, encrypt};

fn main() {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "fixtures/generated".into()),
    );
    std::fs::create_dir_all(&dir).expect("create output directory");
    let photo = image::DynamicImage::ImageRgb8(f::photo(1200, 800, 5));
    let sig = image::DynamicImage::ImageRgba8(f::with_alpha(&f::line_art(300, 120)));
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("text-3.pdf", f::text_pdf(3)),
        ("notes.md", f::markdown()),
        ("text-5.pdf", f::text_pdf(5)),
        ("photo.pdf", f::photo_pdf()),
        ("lineart.pdf", f::lineart_pdf()),
        ("outlined.pdf", f::outlined_pdf()),
        ("form.pdf", f::form_pdf()),
        ("corrupt.pdf", f::corrupt_pdf()),
        (
            "locked.pdf",
            encrypt(
                &f::text_pdf(2),
                &EncryptOptions {
                    user_password: "secret".into(),
                    ..Default::default()
                },
            )
            .expect("encrypt"),
        ),
        ("scan.pdf", scan(&f::text_pdf(3))),
        ("photo.jpg", f::jpeg(&f::photo(1200, 800, 5), 95)),
        ("photo.png", f::png(&photo)),
        ("signature.png", f::png(&sig)),
    ];
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes).expect("write fixture");
    }
    println!("{}", dir.display());
}

/// A "scanned" document: every page of `pdf` as a 150-DPI gray JPEG, no text layer.
fn scan(pdf: &[u8]) -> Vec<u8> {
    let doc = pdfvault_render::open(pdf, None).expect("open");
    let pages: Vec<_> = (0..doc.pages().len())
        .map(|i| {
            let img = pdfvault_render::render_page(&doc, i, 150.0).expect("render");
            let gray = image::DynamicImage::ImageRgba8(img).to_luma8();
            let meta = pdfvault_core::compress::PageImage {
                width: gray.width(),
                height: gray.height(),
                gray: true,
            };
            let jpeg =
                pdfvault_core::raster::encode_jpeg(&pdfvault_core::raster::Pixels::Gray(gray), 80)
                    .expect("jpeg");
            (meta, jpeg)
        })
        .collect();
    pdfvault_core::compress::rebuild_from_page_images(pdf, None, &pages).expect("rebuild")
}
