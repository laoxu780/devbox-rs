use std::process::Command;

use rxing::{
    BinaryBitmap, DecodeHints, Luma8LuminanceSource, MultiUseMultiFormatReader,
    common::HybridBinarizer,
    multi::{GenericMultipleBarcodeReader, MultipleBarcodeReader},
};

use barcoders::sym::{
    codabar::Codabar,
    code128::Code128,
    code39::Code39,
    ean13::{ENCODINGS as EAN_ENCODINGS, LEFT_GUARD, MIDDLE_GUARD, RIGHT_GUARD},
    tf::TF,
};

use gpui::{ClickEvent, Context, Entity, IntoElement, div, prelude::*, px};

use crate::ui::app::{DevBoxApp, VIOLET};
use crate::ui::pages::{ footer_line, io_head, panel_actions, segmented, text_area, tool_page, chip };
use crate::ui::text_input::TextInput;
use crate::ui::widgets::{ ButtonKind, button, card, empty_state, page_header, section_header, status_line };

pub struct CodeState {
    pub text: Entity<TextInput>,
    pub kind: usize,
    pub decoded: String,
    pub error: String,
}

impl CodeState {
    pub fn new(cx: &mut Context<DevBoxApp>) -> Self {
        Self {
            text: cx.new(|cx| TextInput::multi_with(cx, "输入要生成二维码的内容…", "https://devbox.local")),
            kind: 0,
            decoded: String::new(),
            error: String::new(),
        }
    }
}

/// Barcode symbologies available in the generator. The index is persisted in
/// [`CodeState::kind`].
/// Parity of the six left-hand digits of an EAN-13, keyed by its first digit.
const EAN13_PARITY: [[usize; 6]; 10] = [
    [0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 1, 1],
    [0, 0, 1, 1, 0, 1],
    [0, 0, 1, 1, 1, 0],
    [0, 1, 0, 0, 1, 1],
    [0, 1, 1, 0, 0, 1],
    [0, 1, 1, 1, 0, 0],
    [0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 0],
    [0, 1, 1, 0, 1, 0],
];

pub const BARCODE_KINDS: &[(&str, &str)] = &[
    ("二维码", "https://devbox.local"),
    ("Code 39", "HELLO-123"),
    ("Code 128", "HELLO-123"),
    ("EAN-13", "590123412345"),
    ("EAN-8", "9638507"),
    ("UPC-A", "03600029145"),
    ("ITF-14", "1001234567890"),
    ("Codabar", "A123456B"),
];

fn digits_only(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect()
}

fn ean_checksum(digits: &[u8], ean13: bool) -> u8 {
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(index, digit)| {
            let weight = if ean13 {
                if index % 2 == 0 { 1 } else { 3 }
            } else if index % 2 == 0 {
                3
            } else {
                1
            };
            (*digit as u32) * weight
        })
        .sum();
    ((10 - (sum % 10)) % 10) as u8
}

fn encode_ean13(digits: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(95);
    bits.extend_from_slice(&LEFT_GUARD);
    let parity = EAN13_PARITY[digits[0] as usize];
    for (index, digit) in digits[1..7].iter().enumerate() {
        bits.extend_from_slice(&EAN_ENCODINGS[parity[index]][*digit as usize]);
    }
    bits.extend_from_slice(&MIDDLE_GUARD);
    for digit in &digits[7..13] {
        bits.extend_from_slice(&EAN_ENCODINGS[2][*digit as usize]);
    }
    bits.extend_from_slice(&RIGHT_GUARD);
    bits
}

fn encode_ean8(digits: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(67);
    bits.extend_from_slice(&LEFT_GUARD);
    for digit in &digits[0..4] {
        bits.extend_from_slice(&EAN_ENCODINGS[0][*digit as usize]);
    }
    bits.extend_from_slice(&MIDDLE_GUARD);
    for digit in &digits[4..8] {
        bits.extend_from_slice(&EAN_ENCODINGS[2][*digit as usize]);
    }
    bits.extend_from_slice(&RIGHT_GUARD);
    bits
}

/// Encode text into alternating bar/space modules (1 = bar, 0 = space).
fn encode_barcode(kind: usize, text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("请输入条码内容".into());
    }
    let message = |error: barcoders::error::Error| format!("编码失败：{error}");
    match kind {
        1 => Code39::new(text).map(|code| code.encode()).map_err(message),
        2 => Code128::new(format!("Ɓ{text}")).map(|code| code.encode()).map_err(message),
        3 | 5 => {
            let raw = digits_only(text);
            let mut digits: Vec<u8> = raw
                .chars()
                .map(|ch| ch.to_digit(10).unwrap() as u8)
                .collect();
            if kind == 5 {
                // UPC-A is an EAN-13 with number system 0.
                if digits.len() > 12 {
                    return Err("UPC-A 需要 11 或 12 位数字".into());
                }
                if digits.len() == 12 {
                    digits.remove(0);
                }
                if digits.len() != 11 {
                    return Err("UPC-A 需要 11 或 12 位数字".into());
                }
                digits.insert(0, 0);
            } else if digits.len() > 13 {
                return Err("EAN-13 需要 12 或 13 位数字".into());
            } else if digits.len() == 13 {
                digits.truncate(12);
            }
            if digits.len() != 12 {
                return Err("EAN-13 需要 12 或 13 位数字".into());
            }
            digits.push(ean_checksum(&digits, true));
            Ok(encode_ean13(&digits))
        }
        4 => {
            let raw = digits_only(text);
            let mut digits: Vec<u8> = raw
                .chars()
                .map(|ch| ch.to_digit(10).unwrap() as u8)
                .collect();
            if digits.len() > 8 {
                return Err("EAN-8 需要 7 或 8 位数字".into());
            }
            if digits.len() == 8 {
                digits.truncate(7);
            }
            if digits.len() != 7 {
                return Err("EAN-8 需要 7 或 8 位数字".into());
            }
            digits.push(ean_checksum(&digits, false));
            Ok(encode_ean8(&digits))
        }
        6 => {
            let raw = digits_only(text);
            if raw.len() != 13 && raw.len() != 14 {
                return Err("ITF-14 需要 13 或 14 位数字".into());
            }
            TF::interleaved(&raw)
                .map(|code| code.encode())
                .map_err(message)
        }
        7 => Codabar::new(text)
            .map(|code| code.encode())
            .map_err(message),
        _ => Err("不支持的条码类型".into()),
    }
}

fn draw_qr(
    bounds: gpui::Bounds<gpui::Pixels>,
    window: &mut gpui::Window,
    colors: &[qrcode::Color],
    width: usize,
) {
    let side = f32::from(bounds.size.width).min(f32::from(bounds.size.height));
    let quiet = 8.0;
    let module = ((side - quiet * 2.0) / width as f32).floor().max(1.0);
    let origin_x = f32::from(bounds.left()) + (side - module * width as f32) / 2.0;
    let origin_y = f32::from(bounds.top()) + (side - module * width as f32) / 2.0;
    window.paint_quad(gpui::fill(bounds, gpui::white()));
    for (index, color) in colors.iter().enumerate() {
        if matches!(color, qrcode::Color::Dark) {
            let x = origin_x + (index % width) as f32 * module;
            let y = origin_y + (index / width) as f32 * module;
            window.paint_quad(gpui::fill(
                gpui::Bounds::new(
                    gpui::point(gpui::px(x), gpui::px(y)),
                    gpui::size(gpui::px(module), gpui::px(module)),
                ),
                gpui::rgb(0x17181b),
            ));
        }
    }
}

fn draw_bits(
    bounds: gpui::Bounds<gpui::Pixels>,
    window: &mut gpui::Window,
    bits: &[u8],
) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let quiet = 12.0;
    let unit = ((width - quiet * 2.0) / bits.len() as f32).max(0.5);
    let bar_height = (height - 26.0).max(20.0);
    let mut x = f32::from(bounds.left()) + quiet + (width - quiet * 2.0 - unit * bits.len() as f32) / 2.0;
    let y = f32::from(bounds.top()) + 8.0;
    window.paint_quad(gpui::fill(bounds, gpui::white()));
    for (index, is_bar) in bits.iter().enumerate() {
        if *is_bar == 1 {
            window.paint_quad(gpui::fill(
                gpui::Bounds::new(
                    gpui::point(gpui::px(x), gpui::px(y)),
                    gpui::size(gpui::px(unit), gpui::px(bar_height)),
                ),
                gpui::rgb(0x17181b),
            ));
        }
        let _ = index;
        x += unit;
    }
}


/// Decode every barcode/QR code found in an image. Returns all texts joined
/// by newlines, de-duplicated while preserving their order.
fn decode_barcodes(image: image::DynamicImage) -> Result<String, String> {
    let luma = image.to_luma8();
    let (width, height) = luma.dimensions();
    let mut hints = DecodeHints::default();
    hints.TryHarder = Some(true);
    let source = Luma8LuminanceSource::new(luma.into_raw(), width, height)
        .map_err(|error| format!("图像预处理失败：{error}"))?;
    let mut bitmap = BinaryBitmap::new(HybridBinarizer::new(source));
    let mut scanner = GenericMultipleBarcodeReader::new(MultiUseMultiFormatReader::default());
    let results = scanner
        .decode_multiple_with_hints(&mut bitmap, &hints)
        .map_err(|error| format!("解码失败：{error}"))?;

    let mut texts: Vec<String> = Vec::new();
    for result in results {
        let text = result.getText().trim().to_string();
        if !text.is_empty() && !texts.contains(&text) {
            texts.push(text);
        }
    }
    if texts.is_empty() {
        return Err("图片中未识别到条码或二维码".into());
    }
    Ok(texts.join("\n"))
}

fn decode_any(image: image::DynamicImage) -> Result<String, String> {
    decode_barcodes(image.clone()).or_else(|_| decode_qr_image(image))
}

fn decode_qr_image(image: image::DynamicImage) -> Result<String, String> {
    let gray = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let grids = prepared.detect_grids();
    let Some(grid) = grids.first() else {
        return Err("图片中未检测到二维码".into());
    };
    let (_meta, content) = grid
        .decode()
        .map_err(|error| format!("解码失败：{error}"))?;
    Ok(content)
}

fn pdf_to_images(path: &str) -> Result<(std::path::PathBuf, Vec<std::path::PathBuf>), String> {
    let dir = std::env::temp_dir().join(format!("devbox-pdf-{}", crate::util::new_id()));
    std::fs::create_dir_all(&dir).map_err(|error| format!("创建临时目录失败：{error}"))?;

    let poppler = Command::new("pdftoppm")
        .args(["-r", "220", "-png", path])
        .arg(dir.join("page"))
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !poppler {
        // macOS Quick Look is always available and renders at a fixed size.
        let _ = Command::new("qlmanage")
            .args(["-t", "-s", "2400", "-o"])
            .arg(&dir)
            .arg(path)
            .output();
    }

    let mut images: Vec<_> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    matches!(
                        path.extension().and_then(|ext| ext.to_str()),
                        Some("png") | Some("jpg") | Some("jpeg")
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    images.sort();
    if images.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err("无法从 PDF 渲染页面，请安装 poppler 或改用图片".into());
    }
    Ok((dir, images))
}

pub fn decode_file(path: &str) -> Result<String, String> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        let (dir, images) = pdf_to_images(path)?;
        let mut result = Err("PDF 页面中未检测到二维码".into());
        for image_path in &images {
            if let Ok(image) = image::open(image_path) {
                result = decode_any(image);
                if result.is_ok() {
                    break;
                }
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        result
    } else {
        let image = image::open(path).map_err(|error| format!("读取图片失败：{error}"))?;
        decode_any(image)
    }
}

fn pick_image() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        return crate::util::pick_file_path(
            "选择包含二维码的图片或 PDF",
            &["pdf", "png", "jpg", "jpeg", "tiff"],
        );
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose file of type {\"pdf\", \"png\", \"jpg\", \"jpeg\", \"tiff\"} with prompt \"选择包含二维码的图片或 PDF\")",
            ])
            .output()
            .map_err(|error| format!("打开文件选择器失败：{error}"))?;
        if !output.status.success() {
            return Err(String::new());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}


impl DevBoxApp {
    pub fn render_code(&mut self, cx: &mut Context<Self>) -> impl IntoElement {

        let seq = std::cell::Cell::new(0);        let theme = self.theme;
        let text = self.pages.code.text.read(cx).text().to_string();
        let kind = self.pages.code.kind;
        let decoded = self.pages.code.decoded.clone();
        let error = self.pages.code.error.clone();

        let mut mode = segmented(&theme).flex_wrap();
        for (index, (label, _placeholder)) in BARCODE_KINDS.iter().enumerate() {
            mode = mode.child(chip(&theme, *label, kind == index).id(crate::ui::loop_id("code_auto2_1", &seq))
.on_click(cx.listener(
                move |app, _: &ClickEvent, _, cx| {
                    app.pages.code.kind = index;
                    let hint = BARCODE_KINDS[index].1;
                    app.pages.code.text.update(cx, |input, cx| {
                        if input.text().trim().is_empty() {
                            input.set_text(hint, cx);
                        }
                    });
                    cx.notify();
                },
            )));
        }

        let preview = if text.trim().is_empty() {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(360.0))
                .bg(theme.card())
                .border_1()
                .border_color(theme.line())
                .rounded(px(12.0))
                .text_size(px(11.5))
                .text_color(theme.faint())
                .child("输入内容后自动生成")
                .into_any_element()
        } else if kind == 0 {
            match qrcode::QrCode::new(text.as_bytes()) {
                Ok(code) => {
                    let width = code.width();
                    let colors = code.to_colors();
                    div()
                        .size(px(360.0))
                        .bg(gpui::white())
                        .border_1()
                        .border_color(theme.line())
                        .rounded(px(12.0))
                        .overflow_hidden()
                        .child(gpui::canvas(
                            move |bounds, _, _| bounds,
                            move |bounds, _, window, _| {
                                draw_qr(bounds, window, &colors, width);
                            },
                        ).size_full())
                        .into_any_element()
                }
                Err(error) => div()
                    .p(px(12.0))
                    .text_size(px(11.0))
                    .text_color(theme.danger())
                    .child(format!("生成失败：{error}"))
                    .into_any_element(),
            }
        } else {
            match encode_barcode(kind, &text) {
                Ok(bits) => div()
                    .w(px(620.0))
                    .h(px(200.0))
                    .bg(gpui::white())
                    .border_1()
                    .border_color(theme.line())
                    .rounded(px(12.0))
                    .overflow_hidden()
                    .child(gpui::canvas(
                        move |bounds, _, _| bounds,
                        move |bounds, _, window, _| {
                            draw_bits(bounds, window, &bits);
                        },
                    ).size_full())
                    .into_any_element(),
                Err(error) => div()
                    .p(px(12.0))
                    .text_size(px(11.0))
                    .text_color(theme.danger())
                    .child(error)
                    .into_any_element(),
            }
        };

        let mut decode_card = card(&theme);
        decode_card = decode_card.child(section_header(
            &theme,
            "图片 / PDF 解码",
            "识别其中的二维码与条码（Code 128、EAN、UPC 等），多个结果逐行显示",
        ));
        if decoded.is_empty() && error.is_empty() {
            decode_card = decode_card.child(empty_state(&theme, "尚未选择图片或 PDF"));
        } else if !error.is_empty() {
            decode_card = decode_card.child(status_line(&theme, error.clone(), true));
        } else {
            decode_card = decode_card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .p(px(9.0))
                            .bg(theme.inset())
                            .rounded(px(7.0))
                            .font_family("Menlo")
                            .text_size(px(11.5))
                            .text_color(theme.text())
                            .child(decoded.clone()),
                    )
                    .child(
                        button(&theme, "复制", ButtonKind::Ghost).id("code_auto2_2")
.on_click(cx.listener(
                            |app, _: &ClickEvent, _, cx| {
                                let text = app.pages.code.decoded.clone();
                                app.copy(text, cx);
                            },
                        )),
                    ),
            );
        }

        tool_page(&theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(page_header(
                        &theme,
                        "▦",
                        VIOLET,
                        "二维码 / 条码",
                        "生成二维码与 Code 39 条码，图片扫码解码",
                    ))
                    .child(
                        panel_actions(&theme)
                            .child(mode)
                            .child(
                                button(&theme, "选择图片 / PDF 解码", ButtonKind::Ghost).id("code_auto2_3")
.on_click(
                                    cx.listener(|app, _: &ClickEvent, _, cx| {
                                        match pick_image() {
                                            Ok(path) if !path.is_empty() => {
                                                match decode_file(&path) {
                                                    Ok(content) => {
                                                        app.pages.code.decoded = content;
                                                        app.pages.code.error.clear();
                                                        app.notify("解码成功", false, cx);
                                                    }
                                                    Err(error) => {
                                                        app.pages.code.error = error.clone();
                                                        app.notify(error, true, cx);
                                                    }
                                                }
                                            }
                                            Ok(_) => {}
                                            Err(error) => app.notify(error, true, cx),
                                        }
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .flex_1()
                            .min_w_0()
                            .child(io_head(
                                &theme,
                                "内容",
                                format!("{} 字符", text.chars().count()),
                            ))
                            .child(text_area(&theme, &self.pages.code.text, 150.0))
                            .child(decode_card),
                    )
                    .child(div().flex().items_start().child(preview)),
            )
            .child(
                footer_line(&theme)
                    .child(if text.trim().is_empty() {
                        status_line(&theme, "等待输入", false)
                    } else {
                        status_line(&theme, "实时生成 · 本地渲染", false)
                    })
                    .child(div().child("不上传内容")),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_all_supported_barcodes() {
        let cases = [
            (1, "HELLO-123"),
            (2, "HELLO-123"),
            (3, "590123412345"),
            (4, "9638507"),
            (5, "03600029145"),
            (6, "1001234567890"),
            (7, "A123456B"),
        ];
        for (kind, text) in cases {
            let bits = encode_barcode(kind, text)
                .unwrap_or_else(|error| panic!("kind {kind} failed: {error}"));
            assert!(bits.len() > 20, "kind {kind} produced only {} modules", bits.len());
            assert!(bits.iter().any(|bit| *bit == 1), "kind {kind} has no bars");
            assert!(bits.iter().any(|bit| *bit == 0), "kind {kind} has no spaces");
        }
    }

    #[test]
    fn fixed_length_symbologies_validate_input() {
        assert!(encode_barcode(3, "123").is_err());
        assert!(encode_barcode(4, "12").is_err());
        assert!(encode_barcode(5, "123").is_err());
        assert!(encode_barcode(6, "123").is_err());
        assert!(encode_barcode(1, "").is_err());
    }

    #[test]
    fn ean13_has_standard_width() {
        let bits = encode_barcode(3, "5901234123456").unwrap();
        assert_eq!(bits.len(), 95, "EAN-13 must be 95 modules");
        assert_eq!(bits[0], 1);
        assert_eq!(bits[94], 1);
    }
}
