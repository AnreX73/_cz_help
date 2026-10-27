use base64::Engine;
use image::{DynamicImage, RgbImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::Path;

pub struct ImportOutcome {
    pub sha256: String,
    pub rel_path: String,
    pub width: i64,
    pub height: i64,
    pub number: Option<String>,
    pub inn: Option<String>,
    pub payload: Option<String>,
    pub preview_data_url: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

/// Декодирует изображение, композитит прозрачность на белый и сохраняет
/// нормализованную копию в папку вложений.
pub fn import_bytes(bytes: &[u8], attachments: &Path) -> Result<ImportOutcome, String> {
    let img = image::load_from_memory(bytes).map_err(|e| {
        if bytes.len() > 20 * 1024 * 1024 {
            "Файл слишком большой (больше 20 МБ)".to_string()
        } else {
            format!("Не удалось прочитать изображение: {e}")
        }
    })?;

    let (qr_number, qr_inn, payload) = decode_qr(&img);
    let (width, height, rgba) = {
        let rgba = img.to_rgba8();
        (rgba.width(), rgba.height(), rgba)
    };
    if width == 0 || height == 0 {
        return Err("Пустое изображение".into());
    }
    if width > 12_000 || height > 12_000 {
        return Err("Слишком большое изображение".into());
    }

    let normalized = flatten_to_rgb(&rgba);
    let mut png = Vec::new();
    DynamicImage::ImageRgb8(normalized.clone())
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Не удалось сохранить изображение: {e}"))?;

    let sha = sha256_hex(bytes);
    std::fs::create_dir_all(attachments).map_err(|e| e.to_string())?;
    let rel = format!("{sha}.png");
    let path = attachments.join(&rel);
    if !path.exists() {
        std::fs::write(&path, &png).map_err(|e| e.to_string())?;
    }

    let preview = base64::engine::general_purpose::STANDARD.encode(&png);
    Ok(ImportOutcome {
        sha256: sha,
        rel_path: rel,
        width: width as i64,
        height: height as i64,
        number: qr_number,
        inn: qr_inn,
        payload,
        preview_data_url: format!("data:image/png;base64,{preview}"),
    })
}

fn flatten_to_rgb(rgba: &image::RgbaImage) -> RgbImage {
    let (w, h) = rgba.dimensions();
    let mut out = RgbImage::new(w, h);
    for (x, y, px) in rgba.enumerate_pixels() {
        let a = px[3] as u32;
        let blend = |c: u8| -> u8 {
            ((c as u32 * a + 255 * (255 - a)) / 255) as u8
        };
        out.put_pixel(x, y, image::Rgb([blend(px[0]), blend(px[1]), blend(px[2])]));
    }
    out
}

fn decode_qr(img: &DynamicImage) -> (Option<String>, Option<String>, Option<String>) {
    let gray = img.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let grids = prepared.detect_grids();
    for grid in grids {
        if let Ok((_meta, content)) = grid.decode() {
            let (inn, number) = parse_receipt_url(&content);
            if number.is_some() {
                return (number, inn, Some(content));
            }
        }
    }
    (None, None, None)
}

/// `https://lknpd.nalog.ru/api/v1/receipt/540230136541/23cgmf8snp/print`
pub fn parse_receipt_url(url: &str) -> (Option<String>, Option<String>) {
    let trimmed = url.trim();
    let Some(pos) = trimmed.find("/receipt/") else {
        return (None, None);
    };
    let rest = &trimmed[pos + "/receipt/".len()..];
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let inn = parts.next().map(|s| s.to_string());
    let number = parts.next().map(|s| s.to_string());
    match (inn, number) {
        (Some(i), Some(n)) if i.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => {
            (Some(i), Some(n))
        }
        _ => (None, None),
    }
}

pub fn load_receipt(path: &Path) -> Result<(usize, usize, Vec<u8>), String> {
    let img = image::open(path).map_err(|e| format!("Не удалось открыть чек: {e}"))?;
    let rgb = img.to_rgb8();
    Ok((rgb.width() as usize, rgb.height() as usize, rgb.into_raw()))
}

pub fn data_url_for_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Файл недоступен: {e}"))?;
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else {
        "image/jpeg"
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

/// Уменьшенная копия чека для списка документов.
pub fn thumbnail_data_url(path: &Path, max_width: u32) -> Result<String, String> {
    let img = image::open(path).map_err(|e| format!("Файл недоступен: {e}"))?;
    let thumb = img.thumbnail(max_width.max(40), u32::MAX);
    let mut png = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Не удалось уменьшить чек: {e}"))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
    Ok(format!("data:image/png;base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_receipt_url() {
        let (inn, number) = parse_receipt_url("https://lknpd.nalog.ru/api/v1/receipt/540230136541/23cgmf8snp/print");
        assert_eq!(inn.as_deref(), Some("540230136541"));
        assert_eq!(number.as_deref(), Some("23cgmf8snp"));
    }

    #[test]
    fn parses_receipt_url_without_print() {
        let (inn, number) = parse_receipt_url("https://lknpd.nalog.ru/api/v1/receipt/540230136541/23cgmf8snp");
        assert_eq!(inn.as_deref(), Some("540230136541"));
        assert_eq!(number.as_deref(), Some("23cgmf8snp"));
    }

    #[test]
    fn rejects_foreign_payload() {
        let (inn, number) = parse_receipt_url("https://example.com/foo");
        assert!(inn.is_none());
        assert!(number.is_none());
    }

    /// Проверяет распознавание QR на локальных образцах из `samples/`.
    /// Если папки нет (чистый клон), тест ничего не делает.
    #[test]
    fn decodes_qr_in_samples() {
        let dir = std::path::Path::new("../samples");
        if !dir.exists() {
            return;
        }
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).expect("samples") {
            let path = entry.expect("entry").path();
            let is_png = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("png"));
            if !is_png {
                continue;
            }
            let img = image::open(&path).expect("открыть образец");
            let (number, inn, payload) = decode_qr(&img);
            assert!(number.is_some(), "QR не распознан: {}", path.display());
            assert_eq!(inn.as_deref(), Some("540230136541"), "{}", path.display());
            assert!(payload.is_some_and(|p| p.contains("/receipt/")), "{}", path.display());
            checked += 1;
        }
        println!("Проверено чеков: {checked}");
    }
}
