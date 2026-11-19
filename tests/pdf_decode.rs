use image::{ImageBuffer, Luma};

fn write_qr_png(path: &std::path::Path, content: &str) {
    let code = qrcode::QrCode::new(content.as_bytes()).unwrap();
    let colors = code.to_colors();
    let width = code.width();
    let scale = 8u32;
    let quiet = 4u32;
    let size = (width as u32 + quiet * 2) * scale;
    let mut image = ImageBuffer::from_pixel(size, size, Luma([255u8]));
    for (index, color) in colors.iter().enumerate() {
        if matches!(color, qrcode::Color::Dark) {
            let x = quiet + (index % width) as u32;
            let y = quiet + (index / width) as u32;
            for dy in 0..scale {
                for dx in 0..scale {
                    image.put_pixel(x * scale + dx, y * scale + dy, Luma([0u8]));
                }
            }
        }
    }
    image.save(path).unwrap();
}

#[test]
fn decodes_qr_code_from_pdf() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("qr.png");
    let pdf = dir.path().join("qr.pdf");
    let content = "https://devbox.local/pdf-test";
    write_qr_png(&png, content);

    let status = std::process::Command::new("sips")
        .arg("-s")
        .arg("format")
        .arg("pdf")
        .arg(&png)
        .arg("--out")
        .arg(&pdf)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    if !status.success() || !pdf.exists() {
        eprintln!("sips could not create a PDF; skipping PDF decode test");
        return;
    }

    let decoded = devbox::ui::pages::code::decode_file(pdf.to_str().unwrap())
        .expect("PDF decode failed");
    assert_eq!(decoded, content);
}
