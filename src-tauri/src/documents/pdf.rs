use crate::documents::words::amount_in_words;
use crate::models::{Document, Seller, Settings};
use printpdf::{
    Color, FontId, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, RawImage,
    RawImageData, RawImageFormat, Rgb, TextItem, XObjectId, XObjectTransform,
};
use ttf_parser::Face;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN_X: f32 = 15.0;
const MARGIN_TOP: f32 = 16.0;
const MARGIN_BOTTOM: f32 = 14.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN_X;

const FONT_REGULAR: &[u8] = include_bytes!("../../resources/fonts/PT_Sans-Web-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../resources/fonts/PT_Sans-Web-Bold.ttf");

const MM_PT: f32 = 72.0 / 25.4;

const C_TEXT: (f32, f32, f32) = (0.055, 0.09, 0.15);
const C_MUTED: (f32, f32, f32) = (0.42, 0.47, 0.53);
const C_LINE: (f32, f32, f32) = (0.78, 0.81, 0.85);
const C_HEAD_BG: (f32, f32, f32) = (0.949, 0.957, 0.969);
const C_ACCENT: (f32, f32, f32) = (0.78, 0.48, 0.09);

const LEGAL_TITLE: &str = "Важная правовая информация для покупателя:";
const LEGAL_BODY: &str = "Настоящий документ подготовлен исключительно для удобства клиента, \
носит информационный характер и не является первичным учётным или расчётным документом.";
const RECEIPT_NOTE: &str = "Данный чек является официальным подтверждением оплаты и \
предоставления услуги и сформирован в соответствии со ст. 14 Федерального закона № 422-ФЗ \
в приложении «Мой налог».";
const PENDING_TITLE: &str = "Чек НПД не приложен";
const PENDING_BODY: &str = "Чек НПД не приложен по техническим причинам \
(отсутствие интернет-соединения, отсутствие связи с порталом «Мой налог» и прочее). \
Обязуюсь направить чек покупателю по электронной почте или в приложении «MAX» \
сразу после решения технических проблем.";

pub struct ReceiptImage {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

pub struct BuildInput<'a> {
    pub doc: &'a Document,
    pub settings: &'a Settings,
    pub seller: &'a Seller,
    pub receipt: Option<ReceiptImage>,
    pub qr_payload: Option<String>,
}

struct Fonts {
    regular: FontId,
    bold: FontId,
    face_regular: Face<'static>,
    face_bold: Face<'static>,
}

impl Fonts {
    fn measure(&self, text: &str, size: f32, bold: bool) -> f32 {
        let face = if bold { &self.face_bold } else { &self.face_regular };
        let upem = face.units_per_em() as f32;
        let mut width = 0.0f32;
        for ch in text.chars() {
            if let Some(gid) = face.glyph_index(ch) {
                width += face.glyph_hor_advance(gid).unwrap_or(0) as f32;
            }
        }
        width / upem * size
    }
}

pub fn mm(v: f32) -> Pt {
    Pt(v * MM_PT)
}

fn pt_to_mm(v: f32) -> f32 {
    v / MM_PT
}

fn color(c: (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb {
        r: c.0,
        g: c.1,
        b: c.2,
        icc_profile: None,
    })
}

pub fn format_money(kop: i64) -> String {
    let sign = if kop < 0 { "-" } else { "" };
    let abs = kop.abs();
    format!("{sign}{},{:02}", group_thousands(abs / 100), abs % 100)
}

fn group_thousands(n: i64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(*ch as char);
    }
    out
}

fn format_qty(qty_milli: i64) -> String {
    if qty_milli % 1000 == 0 {
        return (qty_milli / 1000).to_string();
    }
    let whole = qty_milli / 1000;
    let mut frac = format!("{:03}", qty_milli % 1000);
    while frac.ends_with('0') {
        frac.pop();
    }
    format!("{whole},{frac}")
}

fn ru_date(iso: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() == 3 {
        format!("{}.{}.{}", parts[2], parts[1], parts[0])
    } else {
        iso.to_string()
    }
}

fn initials(full_name: &str) -> String {
    let parts: Vec<&str> = full_name.split_whitespace().collect();
    match parts.len() {
        0 => String::new(),
        1 => parts[0].to_string(),
        _ => {
            let first = parts[1].chars().next().unwrap_or(' ');
            let third = parts.get(2).and_then(|p| p.chars().next());
            let mut out = parts[0].to_string();
            out.push(' ');
            out.push(first);
            out.push('.');
            if let Some(t) = third {
                out.push(' ');
                out.push(t);
                out.push('.');
            }
            out
        }
    }
}

fn wrap_text(measure: &dyn Fn(&str, f32) -> f32, text: &str, size: f32, max_w: f32) -> Vec<String> {
    // `measure` возвращает ширину в пунктах, а `max_w` задаётся в миллиметрах.
    let max_pt = max_w * MM_PT;
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if measure(&candidate, size) <= max_pt || current.is_empty() {
                current = candidate;
            } else {
                lines.push(current.clone());
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

struct Painter<'f> {
    ops: Vec<Op>,
    fonts: &'f Fonts,
}

impl<'f> Painter<'f> {
    fn new(fonts: &'f Fonts) -> Self {
        Self {
            ops: Vec::new(),
            fonts,
        }
    }

    fn text(&mut self, x: f32, baseline: f32, text: &str, size: f32, bold: bool, c: (f32, f32, f32)) {
        if text.is_empty() {
            return;
        }
        let font = if bold { self.fonts.bold.clone() } else { self.fonts.regular.clone() };
        self.ops.push(Op::StartTextSection);
        self.ops.push(Op::SetFillColor { col: color(c) });
        self.ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(x), Mm(PAGE_H - baseline)),
        });
        self.ops.push(Op::SetFont {
            font: PdfFontHandle::External(font),
            size: Pt(size),
        });
        self.ops.push(Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        });
        self.ops.push(Op::EndTextSection);
    }

    fn text_right(
        &mut self,
        right: f32,
        baseline: f32,
        text: &str,
        size: f32,
        bold: bool,
        c: (f32, f32, f32),
    ) {
        let w = pt_to_mm(self.fonts.measure(text, size, bold));
        self.text(right - w, baseline, text, size, bold, c);
    }

    fn fill_rect(&mut self, x: f32, top: f32, w: f32, h: f32, c: (f32, f32, f32)) {
        self.ops.push(Op::SetFillColor { col: color(c) });
        // В printpdf 0.12 у `Rect` по умолчанию `mode: None`, из-за чего
        // прямоугольник сериализуется как `re n` и ничего не рисует.
        let mut rectangle = printpdf::Rect::from_xywh(mm(x), mm(PAGE_H - top - h), mm(w), mm(h));
        rectangle.mode = Some(printpdf::PaintMode::Fill);
        self.ops.push(Op::DrawRectangle { rectangle });
    }

    fn hline(&mut self, x1: f32, x2: f32, top: f32, weight: f32, c: (f32, f32, f32)) {
        self.fill_rect(x1, top, x2 - x1, weight, c);
    }

    fn image(&mut self, id: XObjectId, x: f32, top: f32, width: f32, px_w: usize, px_h: usize) {
        let scale = width * MM_PT / px_w as f32;
        let height_mm = width * px_h as f32 / px_w as f32;
        self.ops.push(Op::UseXobject {
            id,
            transform: XObjectTransform {
                translate_x: Some(mm(x)),
                translate_y: Some(mm(PAGE_H - top - height_mm)),
                scale_x: Some(scale),
                scale_y: Some(scale),
                dpi: Some(72.0),
                ..Default::default()
            },
        });
    }
}

fn qr_rgb(payload: &str) -> Result<(usize, usize, Vec<u8>), String> {
    let code = qrcode::QrCode::new(payload.as_bytes()).map_err(|e| format!("QR: {e}"))?;
    let modules = code.width();
    let quiet = 4usize;
    let scale = 8usize;
    let side = (modules + quiet * 2) * scale;
    let colors = code.to_colors();
    let mut rgb = vec![255u8; side * side * 3];
    for y in 0..modules {
        for x in 0..modules {
            if colors[y * modules + x] == qrcode::Color::Dark {
                let px0 = (x + quiet) * scale;
                let py0 = (y + quiet) * scale;
                for py in py0..py0 + scale {
                    for px in px0..px0 + scale {
                        let idx = (py * side + px) * 3;
                        rgb[idx] = 0;
                        rgb[idx + 1] = 0;
                        rgb[idx + 2] = 0;
                    }
                }
            }
        }
    }
    Ok((side, side, rgb))
}

fn add_raw_image(doc: &mut PdfDocument, w: usize, h: usize, rgb: Vec<u8>) -> XObjectId {
    let image = RawImage {
        pixels: RawImageData::U8(rgb),
        width: w,
        height: h,
        data_format: RawImageFormat::RGB8,
        tag: Vec::new(),
    };
    doc.add_image(&image)
}

type Column = (f32, f32);

fn columns() -> [Column; 6] {
    [
        (MARGIN_X, 9.0),
        (MARGIN_X + 11.0, 82.0),
        (MARGIN_X + 95.0, 17.0),
        (MARGIN_X + 114.0, 12.0),
        (MARGIN_X + 128.0, 22.0),
        (MARGIN_X + 152.0, 28.0),
    ]
}

struct Metrics {
    scale: f32,
    label: f32,
    body: f32,
    detail: f32,
    name: f32,
    cell: f32,
    line_h: f32,
    pad_v: f32,
    head_h: f32,
    legal: f32,
}

impl Metrics {
    fn new(scale: f32) -> Self {
        Self {
            scale,
            label: 7.2 * scale,
            body: 9.2 * scale,
            detail: 8.0 * scale,
            name: 8.8 * scale,
            cell: 8.6 * scale,
            line_h: 4.1 * scale,
            pad_v: 2.2 * scale,
            head_h: 7.6 * scale,
            legal: (7.4 * scale).max(6.4),
        }
    }
}

fn legal_note_height(fonts: &Fonts, m: &Metrics) -> f32 {
    let title_lines = wrap_text(
        &|t, s| fonts.measure(t, s, false),
        LEGAL_TITLE,
        m.legal,
        CONTENT_W,
    )
    .len();
    let body_lines = wrap_text(
        &|t, s| fonts.measure(t, s, false),
        LEGAL_BODY,
        m.legal - 0.2,
        CONTENT_W,
    )
    .len();
    title_lines as f32 * 4.0 * m.scale + body_lines as f32 * 3.7 * m.scale + 1.8
}

fn draw_legal_note(p: &mut Painter, top: f32, m: &Metrics) -> f32 {
    let mut y = top;
    for line in wrap_text(
        &|t, s| p.fonts.measure(t, s, false),
        LEGAL_TITLE,
        m.legal,
        CONTENT_W,
    ) {
        p.text(MARGIN_X, y, &line, m.legal, true, C_TEXT);
        y += 4.0 * m.scale;
    }
    y += 0.8;
    for line in wrap_text(
        &|t, s| p.fonts.measure(t, s, false),
        LEGAL_BODY,
        m.legal - 0.2,
        CONTENT_W,
    ) {
        p.text(MARGIN_X, y, &line, m.legal - 0.2, false, C_MUTED);
        y += 3.7 * m.scale;
    }
    y + 1.0
}

fn receipt_note_height(fonts: &Fonts, m: &Metrics, width: f32) -> f32 {
    let lines = wrap_text(
        &|t, s| fonts.measure(t, s, false),
        RECEIPT_NOTE,
        m.legal - 0.2,
        width,
    )
    .len();
    lines as f32 * 3.7 * m.scale + 1.0
}

fn draw_receipt_note(p: &mut Painter, top: f32, m: &Metrics, width: f32) -> f32 {
    let mut y = top;
    for line in wrap_text(
        &|t, s| p.fonts.measure(t, s, false),
        RECEIPT_NOTE,
        m.legal - 0.2,
        width,
    ) {
        p.text(MARGIN_X, y, &line, m.legal - 0.2, false, C_MUTED);
        y += 3.7 * m.scale;
    }
    y + 1.0
}

fn receipt_chrome_height(fonts: &Fonts, m: &Metrics) -> f32 {
    6.5 + receipt_note_height(fonts, m, CONTENT_W) + 3.0 + 4.0
}

fn pending_note_height(fonts: &Fonts, m: &Metrics) -> f32 {
    let lines = wrap_text(
        &|t, s| fonts.measure(t, s, false),
        PENDING_BODY,
        m.legal,
        CONTENT_W - 12.0,
    )
    .len();
    5.0 + 4.8 * m.scale + lines as f32 * 3.7 * m.scale + 4.0
}

fn draw_pending_note(p: &mut Painter, top: f32, m: &Metrics) -> f32 {
    let h = pending_note_height(p.fonts, m);
    p.fill_rect(MARGIN_X, top, CONTENT_W, h, C_HEAD_BG);
    p.fill_rect(MARGIN_X, top, 1.2, h, C_ACCENT);
    let mut y = top + 5.0;
    p.text(MARGIN_X + 5.0, y, PENDING_TITLE, m.detail, true, C_TEXT);
    y += 4.8 * m.scale;
    for line in wrap_text(
        &|t, s| p.fonts.measure(t, s, false),
        PENDING_BODY,
        m.legal,
        CONTENT_W - 12.0,
    ) {
        p.text(MARGIN_X + 5.0, y, &line, m.legal, false, C_TEXT);
        y += 3.7 * m.scale;
    }
    top + h
}

/// Высота содержимого без блока чека.
fn content_height(
    fonts: &Fonts,
    doc: &Document,
    seller: &Seller,
    m: &Metrics,
    show_pending: bool,
) -> f32 {
    let mut y = MARGIN_TOP;
    y += 6.5 + 5.0;
    y += legal_note_height(fonts, m);
    y += 5.0;
    y += m.label * 0.64 + 0.8 + 4.6 * m.scale; // метка + имя продавца

    let seller_inn = if doc.seller_inn.is_empty() { &seller.inn } else { &doc.seller_inn };
    let mut details: Vec<String> = Vec::new();
    if !seller_inn.is_empty() {
        details.push(format!("ИНН {seller_inn}"));
    }
    if !seller.status.is_empty() {
        details.push(seller.status.clone());
    }
    if !seller.city.is_empty() {
        details.push(seller.city.clone());
    }
    if !details.is_empty() {
        let details_lines =
            wrap_text(&|t, s| fonts.measure(t, s, false), &details.join(" · "), m.detail, CONTENT_W).len();
        y += details_lines as f32 * 4.0 * m.scale;
    }
    if !seller.contact.is_empty() {
        let contact_lines =
            wrap_text(&|t, s| fonts.measure(t, s, false), &seller.contact, m.detail, CONTENT_W).len();
        y += contact_lines as f32 * 4.0 * m.scale;
    }
    y += 3.0;
    y += m.label * 0.64 + 0.8 + 4.6 * m.scale;

    let buyer = if doc.buyer_name.trim().is_empty() { "—" } else { &doc.buyer_name };
    let buyer_lines = wrap_text(&|t, s| fonts.measure(t, s, false), buyer, m.body, CONTENT_W).len();
    y += buyer_lines as f32 * 4.3 * m.scale;
    if !doc.buyer_inn.trim().is_empty() {
        y += 4.0 * m.scale;
    }
    if !doc.note.trim().is_empty() {
        let note_lines = wrap_text(&|t, s| fonts.measure(t, s, false), doc.note.trim(), m.detail, CONTENT_W).len();
        y += note_lines as f32 * 4.0 * m.scale + 2.0;
    }
    y += 4.0;

    let cols = columns();
    y += m.head_h;
    for item in &doc.items {
        let lines = wrap_text(&|t, s| fonts.measure(t, s, false), &item.name, m.name, cols[1].1 - 1.0);
        y += m.pad_v * 2.0 + lines.len() as f32 * m.line_h;
    }
    if doc.items.is_empty() {
        y += m.pad_v * 2.0 + m.line_h;
    }

    y += 7.2 + 5.2 + 6.2 + 5.2;
    let words = format!("Сумма прописью: {}", amount_in_words(doc.total_kop));
    let words_lines = wrap_text(&|t, s| fonts.measure(t, s, false), &words, m.detail, CONTENT_W).len();
    y += words_lines as f32 * 4.2;
    if show_pending {
        y += 4.0;
        y += pending_note_height(fonts, m);
    }
    y += 8.0 + 3.0;
    y + 2.0
}

pub fn build(input: BuildInput<'_>) -> Result<Vec<u8>, String> {
    let face_regular = Face::parse(FONT_REGULAR, 0).map_err(|e| format!("Шрифт: {e}"))?;
    let face_bold = Face::parse(FONT_BOLD, 0).map_err(|e| format!("Шрифт: {e}"))?;
    let currency: &'static str = if face_regular.glyph_index('₽').is_some() {
        "₽"
    } else {
        "руб."
    };

    let mut pdf = PdfDocument::new("Детализация заказа");
    let parsed_regular = printpdf::ParsedFont::from_bytes(FONT_REGULAR, 0, &mut Vec::new())
        .ok_or_else(|| "Не удалось разобрать шрифт PT Sans".to_string())?;
    let parsed_bold = printpdf::ParsedFont::from_bytes(FONT_BOLD, 0, &mut Vec::new())
        .ok_or_else(|| "Не удалось разобрать шрифт PT Sans Bold".to_string())?;
    let fonts = Fonts {
        regular: pdf.add_font(&parsed_regular),
        bold: pdf.add_font(&parsed_bold),
        face_regular,
        face_bold,
    };

    let doc = input.doc;
    let settings = input.settings;
    let seller = input.seller;

    let receipt_px_w = input.receipt.as_ref().map(|r| r.width).unwrap_or(330);
    let receipt_px_h = input.receipt.as_ref().map(|r| r.height).unwrap_or(546);
    let ratio = receipt_px_h as f32 / receipt_px_w as f32;
    let base_width = settings.receipt_width_mm.clamp(40, 100) as f32;

    // Сначала уменьшаем шрифт, сохраняя ширину чека НПД: QR должен оставаться
    // читаемым. Если не помогло — немного сужаем чек, и только потом переносим
    // его на отдельную страницу.
    let scales = [1.0f32, 0.94, 0.88, 0.82, 0.76];
    let min_width = base_width.min(60.0);
    let mut chosen: Option<(f32, f32)> = None;
    let show_pending = input.receipt.is_none();
    let mut rw = base_width;
    'outer: loop {
        for scale in &scales {
            let m = Metrics::new(*scale);
            let content = content_height(&fonts, doc, seller, &m, show_pending);
            let block = if input.receipt.is_some() {
                rw * ratio + receipt_chrome_height(&fonts, &m)
            } else {
                0.0
            };
            if content + 10.0 <= PAGE_H - MARGIN_BOTTOM - block {
                chosen = Some((*scale, rw));
                break 'outer;
            }
        }
        if rw <= min_width {
            break;
        }
        rw = (rw - 3.0).max(min_width);
    }
    // Не уместилось на одну страницу — разбиваем позиции и уводим чек на
    // отдельную страницу, чтобы ничего не обрезалось.
    let (scale, receipt_w, receipt_own_page) = match chosen {
        Some((scale, width)) => (scale, width, false),
        None => (0.94, base_width, true),
    };

    let receipt_id = input
        .receipt
        .as_ref()
        .map(|r| add_raw_image(&mut pdf, r.width, r.height, r.rgb.clone()));
    let qr = if settings.extra_qr {
        input
            .qr_payload
            .as_deref()
            .and_then(|p| qr_rgb(p).ok())
            .map(|(w, h, rgb)| (add_raw_image(&mut pdf, w, h, rgb), w))
    } else {
        None
    };

    let m = Metrics::new(scale);
    let mut page_ops: Vec<Vec<Op>> = Vec::new();
    let mut p = Painter::new(&fonts);

    let mut y = draw_header(&mut p, doc, seller, &m);
    draw_table_head(&mut p, y, &m);
    y += m.head_h;

    let bottom = PAGE_H - MARGIN_BOTTOM;
    let reserve = totals_height(&fonts, doc, &m, show_pending) + 6.0;

    for (idx, item) in doc.items.iter().enumerate() {
        let lines = wrap_text(&|t, s| p.fonts.measure(t, s, false), &item.name, m.name, columns()[1].1 - 1.0);
        let row_h = m.pad_v * 2.0 + lines.len() as f32 * m.line_h;
        if y + row_h + reserve > bottom {
            page_ops.push(std::mem::take(&mut p.ops));
            p = Painter::new(&fonts);
            y = MARGIN_TOP;
            p.text(MARGIN_X, y, "Детализация заказа — продолжение", m.detail, true, C_MUTED);
            if doc.number.trim().is_empty() {
                p.text_right(
                    PAGE_W - MARGIN_X,
                    y,
                    &format!("от {}", ru_date(&doc.doc_date)),
                    m.detail,
                    false,
                    C_MUTED,
                );
            } else {
                p.text_right(
                    PAGE_W - MARGIN_X,
                    y,
                    &format!("№ {}", doc.number),
                    m.detail,
                    false,
                    C_MUTED,
                );
            }
            y += 6.0;
            draw_table_head(&mut p, y, &m);
            y += m.head_h;
        }
        y = draw_item_row(&mut p, idx, item, &lines, y, &m);
    }
    if doc.items.is_empty() {
        y += m.pad_v * 2.0 + m.line_h;
        p.hline(MARGIN_X, PAGE_W - MARGIN_X, y, 0.2, C_LINE);
    }
    p.hline(MARGIN_X, PAGE_W - MARGIN_X, y, 0.35, C_LINE);

    y = draw_totals(
        &mut p,
        y,
        doc,
        &m,
        currency,
        if doc.seller_name.is_empty() {
            &seller.name
        } else {
            &doc.seller_name
        },
        show_pending,
    );

    // Блок чека НПД: внизу страницы, а если места нет — на отдельной странице.
    if let Some(image_id) = receipt_id {
        let receipt_h = receipt_w * ratio;
        let block_h = receipt_h + receipt_chrome_height(&fonts, &m);
        if receipt_own_page || y + 10.0 + block_h > bottom {
            page_ops.push(std::mem::take(&mut p.ops));
            p = Painter::new(&fonts);
            draw_receipt_page(&mut p, image_id, qr, doc, &m, receipt_w, receipt_px_w, receipt_px_h);
        } else {
            draw_receipt_inline(
                &mut p,
                image_id,
                qr,
                &m,
                receipt_w,
                receipt_px_w,
                receipt_px_h,
            );
        }
    }
    page_ops.push(std::mem::take(&mut p.ops));

    let pages: Vec<PdfPage> = page_ops
        .into_iter()
        .map(|ops| PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops))
        .collect();

    let mut warnings = Vec::new();
    let bytes = pdf.with_pages(pages).save(&PdfSaveOptions::default(), &mut warnings);
    if bytes.len() < 100 {
        return Err("Не удалось сформировать PDF".into());
    }
    Ok(bytes)
}

fn draw_header(p: &mut Painter, doc: &Document, seller: &Seller, m: &Metrics) -> f32 {
    let scale = m.scale;
    let mut y = MARGIN_TOP;
    p.text(MARGIN_X, y, "Детализация заказа", 15.0 * scale.max(0.9), true, C_TEXT);
    if doc.number.trim().is_empty() {
        p.text_right(
            PAGE_W - MARGIN_X,
            y + 0.4,
            &format!("от {}", ru_date(&doc.doc_date)),
            8.5 * scale,
            false,
            C_MUTED,
        );
    } else {
        p.text_right(PAGE_W - MARGIN_X, y - 0.4, &format!("№ {}", doc.number), 12.0 * scale, true, C_TEXT);
        p.text_right(
            PAGE_W - MARGIN_X,
            y + 5.0,
            &format!("от {}", ru_date(&doc.doc_date)),
            8.5 * scale,
            false,
            C_MUTED,
        );
    }
    y += 6.5;
    p.hline(MARGIN_X, PAGE_W - MARGIN_X, y, 0.35, C_LINE);
    y += 5.0;
    y = draw_legal_note(p, y, m);
    y += 5.0;

    p.text(MARGIN_X, y, "ПРОДАВЕЦ", m.label, true, C_MUTED);
    y += m.label * 0.64 + 0.8;
    let seller_name = if doc.seller_name.is_empty() {
        seller.name.clone()
    } else {
        doc.seller_name.clone()
    };
    let seller_inn = if doc.seller_inn.is_empty() {
        seller.inn.clone()
    } else {
        doc.seller_inn.clone()
    };
    p.text(MARGIN_X, y, &seller_name, m.body + 0.6, true, C_TEXT);
    y += 4.6 * scale;

    let mut details: Vec<String> = Vec::new();
    if !seller_inn.is_empty() {
        details.push(format!("ИНН {seller_inn}"));
    }
    if !seller.status.is_empty() {
        details.push(seller.status.clone());
    }
    if !seller.city.is_empty() {
        details.push(seller.city.clone());
    }
    if !details.is_empty() {
        for line in wrap_text(&|t, s| p.fonts.measure(t, s, false), &details.join(" · "), m.detail, CONTENT_W) {
            p.text(MARGIN_X, y, &line, m.detail, false, C_TEXT);
            y += 4.0 * scale;
        }
    }
    if !seller.contact.is_empty() {
        for line in wrap_text(&|t, s| p.fonts.measure(t, s, false), &seller.contact, m.detail, CONTENT_W) {
            p.text(MARGIN_X, y, &line, m.detail, false, C_MUTED);
            y += 4.0 * scale;
        }
    }

    y += 3.0;
    p.text(MARGIN_X, y, "ПОКУПАТЕЛЬ", m.label, true, C_MUTED);
    y += m.label * 0.64 + 0.8;
    let buyer = if doc.buyer_name.trim().is_empty() {
        "—".to_string()
    } else {
        doc.buyer_name.clone()
    };
    for line in wrap_text(&|t, s| p.fonts.measure(t, s, false), &buyer, m.body, CONTENT_W) {
        p.text(MARGIN_X, y, &line, m.body, false, C_TEXT);
        y += 4.3 * scale;
    }
    if !doc.buyer_inn.trim().is_empty() {
        p.text(MARGIN_X, y, &format!("ИНН {}", doc.buyer_inn.trim()), m.detail, false, C_MUTED);
        y += 4.0 * scale;
    }
    if !doc.note.trim().is_empty() {
        for line in wrap_text(&|t, s| p.fonts.measure(t, s, false), doc.note.trim(), m.detail, CONTENT_W) {
            p.text(MARGIN_X, y, &line, m.detail, false, C_MUTED);
            y += 4.0 * scale;
        }
        y += 2.0;
    }
    y + 4.0
}

fn draw_table_head(p: &mut Painter, y: f32, m: &Metrics) {
    let cols = columns();
    p.fill_rect(MARGIN_X, y, CONTENT_W, m.head_h, C_HEAD_BG);
    let head_base = y + m.head_h - 2.6 * m.scale;
    let head_size = 7.4 * m.scale;
    p.text(cols[0].0 + 1.0, head_base, "№", head_size, true, C_MUTED);
    p.text(cols[1].0, head_base, "Наименование", head_size, true, C_MUTED);
    p.text_right(cols[2].0 + cols[2].1, head_base, "Кол-во", head_size, true, C_MUTED);
    p.text(cols[3].0, head_base, "Ед.", head_size, true, C_MUTED);
    p.text_right(cols[4].0 + cols[4].1, head_base, "Цена", head_size, true, C_MUTED);
    p.text_right(cols[5].0 + cols[5].1, head_base, "Сумма", head_size, true, C_MUTED);
}

fn draw_item_row(
    p: &mut Painter,
    idx: usize,
    item: &crate::models::DocumentItem,
    lines: &[String],
    y: f32,
    m: &Metrics,
) -> f32 {
    let cols = columns();
    let row_h = m.pad_v * 2.0 + lines.len() as f32 * m.line_h;
    let mut ly = y + m.pad_v + m.name * 0.86;
    let cell_base = y + m.pad_v + m.cell * 0.86;
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            p.text(cols[0].0 + 1.0, ly, &(idx + 1).to_string(), m.cell, false, C_TEXT);
        }
        p.text(cols[1].0, ly, line, m.name, false, C_TEXT);
        ly += m.line_h;
    }
    p.text_right(cols[2].0 + cols[2].1, cell_base, &format_qty(item.qty_milli), m.cell, false, C_TEXT);
    p.text(cols[3].0, cell_base, &item.unit, m.cell, false, C_MUTED);
    p.text_right(cols[4].0 + cols[4].1, cell_base, &format_money(item.price_kop), m.cell, false, C_TEXT);
    p.text_right(cols[5].0 + cols[5].1, cell_base, &format_money(item.sum_kop), m.cell, false, C_TEXT);
    let next = y + row_h;
    p.hline(MARGIN_X, PAGE_W - MARGIN_X, next, 0.2, C_LINE);
    next
}

fn totals_height(fonts: &Fonts, doc: &Document, m: &Metrics, show_pending: bool) -> f32 {
    let words = format!("Сумма прописью: {}", amount_in_words(doc.total_kop));
    let words_lines = wrap_text(&|t, s| fonts.measure(t, s, false), &words, m.detail, CONTENT_W).len();
    let mut h = 7.2 + 5.2 + 6.2 + 5.2 + words_lines as f32 * 4.2;
    if show_pending {
        h += 4.0 + pending_note_height(fonts, m);
    }
    h + 8.0 + 3.0
}

fn draw_totals(
    p: &mut Painter,
    y: f32,
    doc: &Document,
    m: &Metrics,
    currency: &str,
    seller: &str,
    show_pending: bool,
) -> f32 {
    let mut y = y + 7.2;
    p.text_right(PAGE_W - MARGIN_X - 38.0, y, "Итого:", m.body, true, C_TEXT);
    p.text_right(
        PAGE_W - MARGIN_X,
        y - 0.4,
        &format!("{} {}", format_money(doc.total_kop), currency),
        m.body + 2.0,
        true,
        C_TEXT,
    );
    y += 5.2;
    p.text_right(PAGE_W - MARGIN_X, y, "Налог на профессиональный доход", m.detail, false, C_MUTED);

    y += 6.2;
    let total_line = format!(
        "Всего наименований {}, на сумму {} {}",
        doc.items.len(),
        format_money(doc.total_kop),
        currency
    );
    p.text(MARGIN_X, y, &total_line, m.detail, false, C_TEXT);
    y += 5.2;

    let words = format!("Сумма прописью: {}", amount_in_words(doc.total_kop));
    for line in wrap_text(&|t, s| p.fonts.measure(t, s, false), &words, m.detail, CONTENT_W) {
        p.text(MARGIN_X, y, &line, m.detail, false, C_MUTED);
        y += 4.2;
    }

    if show_pending {
        y += 4.0;
        y = draw_pending_note(p, y, m);
    }

    y += 8.0;
    let sig_base = y;
    p.text(MARGIN_X, sig_base, "Продавец", m.detail, false, C_TEXT);
    p.hline(MARGIN_X + 18.0, MARGIN_X + 76.0, sig_base + 1.2, 0.3, C_TEXT);
    let short = initials(seller);    if !short.is_empty() {
        p.text(MARGIN_X + 79.0, sig_base, &short, m.detail, false, C_TEXT);
    }
    p.text_right(PAGE_W - MARGIN_X, sig_base, "подпись", 7.0 * m.scale, false, C_MUTED);
    sig_base + 3.0
}

fn draw_qr(p: &mut Painter, qr: Option<(XObjectId, usize)>, x: f32, top: f32, m: &Metrics) {
    let Some((qr_id, qr_px)) = qr else { return };
    let qr_size = 22.0f32;
    p.image(qr_id, x, top, qr_size, qr_px, qr_px);
    p.text(x, top + qr_size + 4.0, "Проверить чек", m.detail - 0.4, true, C_TEXT);
    for (i, line) in wrap_text(
        &|t, s| p.fonts.measure(t, s, false),
        "Отсканируйте QR-код, чтобы открыть чек в сервисе ФНС.",
        m.detail - 1.2,
        52.0,
    )
    .iter()
    .enumerate()
    {
        p.text(
            x,
            top + qr_size + 9.0 + i as f32 * 3.8,
            line,
            m.detail - 1.2,
            false,
            C_MUTED,
        );
    }
}

fn receipt_footer(p: &mut Painter, m: &Metrics) {
    p.text(
        MARGIN_X,
        PAGE_H - MARGIN_BOTTOM - 1.0,
        "Детализация заказа сформирована в дополнение к чеку НПД.",
        m.detail - 1.6,
        false,
        C_MUTED,
    );
}

fn draw_receipt_inline(
    p: &mut Painter,
    image_id: XObjectId,
    qr: Option<(XObjectId, usize)>,
    m: &Metrics,
    receipt_w: f32,
    px_w: usize,
    px_h: usize,
) {
    let receipt_h = receipt_w * px_h as f32 / px_w as f32;
    let block_h = receipt_h + receipt_chrome_height(p.fonts, m);
    let top = PAGE_H - MARGIN_BOTTOM - block_h;
    p.hline(MARGIN_X, PAGE_W - MARGIN_X, top - 5.0, 0.35, C_LINE);
    p.text(
        MARGIN_X,
        top + 2.5,
        "Чек, сформированный в приложении «Мой налог»",
        m.detail,
        true,
        C_TEXT,
    );
    let note_end = draw_receipt_note(p, top + 6.5, m, CONTENT_W);
    let img_top = note_end + 3.0;
    p.image(image_id, MARGIN_X, img_top, receipt_w, px_w, px_h);
    draw_qr(p, qr, MARGIN_X + receipt_w + 10.0, img_top + 2.0, m);
    receipt_footer(p, m);
}

fn draw_receipt_page(
    p: &mut Painter,
    image_id: XObjectId,
    qr: Option<(XObjectId, usize)>,
    doc: &Document,
    m: &Metrics,
    receipt_w: f32,
    px_w: usize,
    px_h: usize,
) {
    let mut y = MARGIN_TOP;
    p.text(
        MARGIN_X,
        y,
        "Чек, сформированный в приложении «Мой налог»",
        m.detail + 0.6,
        true,
        C_TEXT,
    );
    y += 5.4;
    p.text(
        MARGIN_X,
        y,
        &format!("К детализации заказа № {} от {}", doc.number, ru_date(&doc.doc_date)),
        m.detail,
        false,
        C_MUTED,
    );
    y += 5.0;
    y = draw_receipt_note(p, y, m, CONTENT_W);
    y += 3.0;
    p.image(image_id, MARGIN_X, y, receipt_w, px_w, px_h);
    draw_qr(p, qr, MARGIN_X + receipt_w + 10.0, y + 2.0, m);
    receipt_footer(p, m);
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DocumentItem;

    #[test]
    fn money_formatting() {
        assert_eq!(format_money(610_00), "610,00");
        assert_eq!(format_money(1_234_567), "12 345,67");
        assert_eq!(format_money(0), "0,00");
    }

    #[test]
    fn qty_formatting() {
        assert_eq!(format_qty(1000), "1");
        assert_eq!(format_qty(1500), "1,5");
        assert_eq!(format_qty(1250), "1,25");
        assert_eq!(format_qty(2000), "2");
    }

    #[test]
    fn initials_parsing() {
        assert_eq!(initials("Иванов Иван Иванович"), "Иванов И. И.");
        assert_eq!(initials("Петров Пётр"), "Петров П.");
    }

    #[test]
    fn dates() {
        assert_eq!(ru_date("2026-10-22"), "22.10.2026");
    }

    #[test]
    fn wrap_uses_millimetres() {
        // Ширина символа 0,5 × размер шрифта в пунктах.
        let measure = |t: &str, size: f32| t.chars().count() as f32 * size * 0.5;
        let long = "Услуга номер один с длинным названием";
        // 37 × 10 × 0,5 = 185 pt ≈ 65 мм — помещается в 80 мм.
        assert_eq!(wrap_text(&measure, long, 10.0, 80.0).len(), 1);
        // В 50 мм (≈142 pt) та же строка не помещается.
        assert!(wrap_text(&measure, long, 10.0, 50.0).len() > 1);
        // Явный перенос строки сохраняется.
        assert_eq!(wrap_text(&measure, "Первая\nВторая", 10.0, 80.0), vec!["Первая", "Вторая"]);
    }

    fn test_document(items: usize) -> Document {
        Document {
            id: 1,
            seller_id: 1,
            number: "23cgmf8snp".into(),
            doc_date: "2026-10-22".into(),
            buyer_name: "ООО «Ромашка»".into(),
            buyer_inn: "7701234567".into(),
            total_kop: 0,
            note: "Оплата по заказу № 12. Спасибо за покупку!".into(),
            seller_name: "Иванов Иван Иванович".into(),
            seller_inn: "540230136541".into(),
            receipt_file_id: Some(1),
            receipt_path: None,
            receipt_pending: false,
            qr_payload: "https://lknpd.nalog.ru/api/v1/receipt/540230136541/23cgmf8snp/print".into(),
            created_at: String::new(),
            items: (0..items)
                .map(|i| DocumentItem {
                    id: 0,
                    document_id: 1,
                    position: i as i64,
                    name: format!("Услуга номер {} с довольно длинным названием для проверки переноса", i + 1),
                    qty_milli: 1000,
                    unit: "усл.".into(),
                    price_kop: 100_00,
                    sum_kop: 100_00,
                })
                .collect(),
        }
    }

    fn test_seller() -> Seller {
        Seller {
            id: 1,
            name: "Иванов Иван Иванович".into(),
            inn: "540230136541".into(),
            contact: "ivanov@example.ru".into(),
            city: "Новосибирск".into(),
            status: "Плательщик налога на профессиональный доход".into(),
            folder: "Иванов_Иван_Иванович-1".into(),
            archived: false,
            created_at: String::new(),
        }
    }

    #[test]
    fn builds_single_and_multi_page_documents() {
        for items in [1usize, 5, 14, 30, 60] {
            let mut doc = test_document(items);
            doc.total_kop = doc.items.iter().map(|i| i.sum_kop).sum();
            let settings = Settings::default();
            let seller = test_seller();
            let receipt = ReceiptImage {
                width: 330,
                height: 546,
                rgb: vec![255u8; 330 * 546 * 3],
            };
            let bytes = build(BuildInput {
                doc: &doc,
                settings: &settings,
                seller: &seller,
                receipt: Some(receipt),
                qr_payload: Some(doc.qr_payload.clone()),
            })
            .unwrap_or_else(|e| panic!("{items} позиций: {e}"));
            assert!(bytes.len() > 5_000, "{items} позиций: PDF слишком мал");
        }
    }

    #[test]
    fn builds_document_without_receipt_and_number() {
        for items in [1usize, 5, 30] {
            let mut doc = test_document(items);
            doc.number = String::new();
            doc.qr_payload = String::new();
            doc.receipt_file_id = None;
            doc.receipt_pending = true;
            doc.total_kop = doc.items.iter().map(|i| i.sum_kop).sum();
            let settings = Settings::default();
            let seller = test_seller();
            let bytes = build(BuildInput {
                doc: &doc,
                settings: &settings,
                seller: &seller,
                receipt: None,
                qr_payload: None,
            })
            .unwrap_or_else(|e| panic!("без чека, {items} позиций: {e}"));
            assert!(bytes.len() > 5_000, "без чека, {items} позиций: PDF слишком мал");
        }
    }
}
