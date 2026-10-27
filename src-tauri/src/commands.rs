use crate::db;
use crate::documents::{import, pdf};
use crate::models::*;
use crate::tax;
use crate::AppState;
use base64::Engine;
use chrono::{Datelike, Local, NaiveDate};
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};
use tauri::State;

type R<T> = Result<T, String>;

fn to_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn lock<'a>(
    state: &'a State<'_, AppState>,
) -> R<std::sync::MutexGuard<'a, rusqlite::Connection>> {
    state
        .db
        .lock()
        .map_err(|_| "База данных временно недоступна".to_string())
}

fn valid_date(value: &str) -> bool {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

fn round_div(n: i128, d: i128) -> i64 {
    if d == 0 {
        0
    } else {
        ((n + d / 2) / d) as i64
    }
}

// ---------- настройки ----------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> R<Settings> {
    let conn = lock(&state)?;
    Ok(db::load_settings(&conn))
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> R<Settings> {
    let conn = lock(&state)?;
    let mut clean = settings;
    clean.rate_individual = clean.rate_individual.clamp(0, 1000);
    clean.rate_organization = clean.rate_organization.clamp(0, 1000);
    clean.receipt_width_mm = clean.receipt_width_mm.clamp(40, 100);
    db::save_settings(&conn, &clean).map_err(to_err)?;
    Ok(db::load_settings(&conn))
}

// ---------- самозанятые ----------

#[tauri::command]
pub fn list_sellers(state: State<'_, AppState>, include_archived: bool) -> R<Vec<Seller>> {
    let conn = lock(&state)?;
    db::list_sellers(&conn, include_archived).map_err(to_err)
}

#[tauri::command]
pub fn active_seller(state: State<'_, AppState>) -> R<Seller> {
    let conn = lock(&state)?;
    db::active_seller(&conn).map_err(to_err)
}

#[tauri::command]
pub fn save_seller(state: State<'_, AppState>, input: SellerInput) -> R<Seller> {
    let conn = lock(&state)?;
    let mut input = input;
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Укажите имя самозанятого".into());
    }
    let inn = input.inn.trim();
    if !inn.is_empty() && (inn.len() != 12 || !inn.chars().all(|c| c.is_ascii_digit())) {
        return Err("ИНН самозанятого — 12 цифр".into());
    }
    // На чистой базе уже есть пустой профиль-заготовка: не плодим «Без имени»,
    // а заполняем его.
    let mut adopted = false;
    if input.id.is_none() {
        if let Ok(all) = db::list_sellers(&conn, true) {
            if all.len() == 1 && all[0].name.trim().is_empty() && all[0].inn.trim().is_empty() {
                input.id = Some(all[0].id);
                adopted = true;
            }
        }
    }
    let mut saved = db::save_seller(&conn, &input).map_err(to_err)?;
    if adopted {
        saved = db::ensure_seller_folder(&conn, saved.id).map_err(to_err)?;
    }
    let active = db::active_seller(&conn).map_err(to_err)?;
    if active.name.trim().is_empty() && active.inn.trim().is_empty() {
        // Первый профиль сразу становится активным.
        db::set_active_seller(&conn, saved.id).map_err(to_err)?;
    }
    db::get_seller(&conn, saved.id).map_err(to_err)
}

#[tauri::command]
pub fn switch_seller(state: State<'_, AppState>, id: i64) -> R<Seller> {
    let conn = lock(&state)?;
    let seller = db::get_seller(&conn, id).map_err(|_| "Самозанятый не найден".to_string())?;
    if seller.archived {
        return Err("Профиль в архиве — сначала верните его из архива".into());
    }
    db::set_active_seller(&conn, id).map_err(to_err)?;
    Ok(seller)
}

#[tauri::command]
pub fn set_seller_archived(state: State<'_, AppState>, id: i64, archived: bool) -> R<()> {
    let conn = lock(&state)?;
    if archived {
        let active = db::active_seller(&conn).map_err(to_err)?;
        if active.id == id {
            return Err("Нельзя отправить в архив активный профиль — переключитесь на другого".into());
        }
        let alive: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sellers WHERE archived = 0 AND id <> ?1",
                params![id],
                |r| r.get(0),
            )
            .map_err(to_err)?;
        if alive == 0 {
            return Err("Нельзя отправить в архив единственного самозанятого".into());
        }
    }
    db::set_seller_archived(&conn, id, archived).map_err(to_err)
}

#[tauri::command]
pub fn delete_seller(state: State<'_, AppState>, id: i64) -> R<()> {
    let conn = lock(&state)?;
    let active = db::active_seller(&conn).map_err(to_err)?;
    if active.id == id {
        return Err("Нельзя удалить активный профиль — сначала переключитесь".into());
    }
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM sellers", [], |r| r.get(0))
        .map_err(to_err)?;
    if total <= 1 {
        return Err("Нельзя удалить единственного самозанятого".into());
    }
    let entries: i64 = conn
        .query_row("SELECT COUNT(*) FROM entries WHERE seller_id = ?1", params![id], |r| r.get(0))
        .map_err(to_err)?;
    let documents: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents WHERE seller_id = ?1", params![id], |r| r.get(0))
        .map_err(to_err)?;
    if entries > 0 || documents > 0 {
        return Err("У самозанятого есть доходы или чеки — отправьте профиль в архив".into());
    }
    db::delete_seller(&conn, id).map_err(to_err)
}

#[tauri::command]
pub fn app_paths(state: State<'_, AppState>) -> R<serde_json::Value> {
    Ok(serde_json::json!({
        "dataDir": state.data_dir.to_string_lossy(),
        "attachmentsDir": state.attachments_dir.to_string_lossy(),
        "documentsDir": state.docs_dir.to_string_lossy(),
        "dbPath": state.db_path.to_string_lossy(),
    }))
}

// ---------- обзор ----------

#[tauri::command]
pub fn get_dashboard(state: State<'_, AppState>) -> R<Dashboard> {
    let conn = lock(&state)?;
    let settings = db::load_settings(&conn);
    let seller = db::active_seller(&conn).map_err(to_err)?;
    let rules = tax::TaxRules::from_settings(&settings);
    let entries = db::all_entries_lite(&conn, seller.id).map_err(to_err)?;
    let months = tax::compute(&entries, &rules);

    let today = Local::now().date_naive();
    let year = today.year() as i64;
    let month = today.month() as i64;

    let (month_income, month_entries) = tax::sum_income(&months, year, month, month);
    let month_tax = tax::sum_tax(&months, year, month, month);
    let q_from = (month - 1) / 3 * 3 + 1;
    let (q_income, q_entries) = tax::sum_income(&months, year, q_from, q_from + 2);
    let q_tax = tax::sum_tax(&months, year, q_from, q_from + 2);
    let y_income = tax::year_income(&months, year);
    let (_, y_entries) = tax::sum_income(&months, year, 1, 12);
    let y_tax = tax::sum_tax(&months, year, 1, 12);

    let deduction_left = tax::deduction_left(&rules, &months);
    let limit_total = settings.limit_kop.max(1);
    let limit_left = (limit_total - y_income).max(0);
    let limit_ratio = (y_income as f64 / limit_total as f64).clamp(0.0, 1.0);

    let (prev_year, prev_month) = if month == 1 { (year - 1, 12) } else { (year, month - 1) };
    let prev_tax = tax::sum_tax(&months, prev_year, prev_month, prev_month);
    let quarter_num = (month - 1) / 3 + 1;
    let quarter_roman = ["I", "II", "III", "IV"][(quarter_num - 1) as usize];
    let due = if prev_tax > 0 {
        let due_date = format!("{year:04}-{month:02}-28");
        let paid: bool = conn
            .query_row(
                "SELECT 1 FROM payments WHERE seller_id = ?1 AND year = ?2 AND month = ?3",
                params![seller.id, prev_year, prev_month],
                |_| Ok(true),
            )
            .optional()
            .map_err(to_err)?
            .unwrap_or(false);
        Some(PaymentDue {
            year: prev_year,
            month: prev_month,
            tax_kop: prev_tax,
            due_date,
            paid,
        })
    } else {
        None
    };

    let recent = db::list_entries(&conn, seller.id, None, None, None, None, Some(8)).map_err(to_err)?;
    let unissued: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entries WHERE seller_id = ?1 AND receipt_issued = 0",
            params![seller.id],
            |r| r.get(0),
        )
        .map_err(to_err)?;
    let documents_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM documents WHERE seller_id = ?1",
            params![seller.id],
            |r| r.get(0),
        )
        .map_err(to_err)?;

    Ok(Dashboard {
        settings,
        seller,
        month: PeriodTotals {
            label: month_label(year, month),
            income_kop: month_income,
            tax_kop: month_tax,
            entries_count: month_entries,
        },
        quarter: PeriodTotals {
            label: format!("{quarter_roman} квартал {year}"),
            income_kop: q_income,
            tax_kop: q_tax,
            entries_count: q_entries,
        },
        year: PeriodTotals {
            label: format!("{year} год"),
            income_kop: y_income,
            tax_kop: y_tax,
            entries_count: y_entries,
        },
        deduction_left_kop: deduction_left,
        deduction_total_kop: rules.deduction_kop,
        limit_left_kop: limit_left,
        limit_total_kop: limit_total,
        limit_ratio,
        recent,
        unissued_receipts: unissued,
        documents_count,
        due,
        months,
    })
}

fn month_label(year: i64, month: i64) -> String {
    let names = [
        "январь", "февраль", "март", "апрель", "май", "июнь", "июль", "август", "сентябрь",
        "октябрь", "ноябрь", "декабрь",
    ];
    let name = names.get((month - 1).max(0) as usize).copied().unwrap_or("");
    format!("{} {}", name, year)
}

#[tauri::command]
pub fn list_months(state: State<'_, AppState>) -> R<Vec<MonthReport>> {
    let conn = lock(&state)?;
    let settings = db::load_settings(&conn);
    let seller = db::active_seller(&conn).map_err(to_err)?;
    let rules = tax::TaxRules::from_settings(&settings);
    let entries = db::all_entries_lite(&conn, seller.id).map_err(to_err)?;
    Ok(tax::compute(&entries, &rules))
}

#[tauri::command]
pub fn set_paid(state: State<'_, AppState>, year: i64, month: i64, paid: bool) -> R<()> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    if paid {
        conn.execute(
            "INSERT INTO payments (seller_id, year, month, amount_kop, paid_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(seller_id, year, month) DO UPDATE SET amount_kop = excluded.amount_kop, paid_at = excluded.paid_at",
            params![seller.id, year, month, 0, db::today()],
        )
        .map_err(to_err)?;
    } else {
        conn.execute(
            "DELETE FROM payments WHERE seller_id = ?1 AND year = ?2 AND month = ?3",
            params![seller.id, year, month],
        )
        .map_err(to_err)?;
    }
    Ok(())
}

// ---------- доходы ----------

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryFilter {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub payer_type: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
}

#[tauri::command]
pub fn list_entries(state: State<'_, AppState>, filter: EntryFilter) -> R<Vec<Entry>> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    db::list_entries(
        &conn,
        seller.id,
        filter.date_from.as_deref(),
        filter.date_to.as_deref(),
        filter.payer_type.as_deref(),
        filter.search.as_deref(),
        filter.limit,
    )
    .map_err(to_err)
}

#[tauri::command]
pub fn save_entry(state: State<'_, AppState>, input: EntryInput) -> R<Entry> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    if !valid_date(&input.date) {
        return Err("Некорректная дата".into());
    }
    if input.amount_kop <= 0 {
        return Err("Сумма должна быть больше нуля".into());
    }
    if input.payer_type != PAYER_INDIVIDUAL && input.payer_type != PAYER_ORGANIZATION {
        return Err("Неизвестный тип плательщика".into());
    }
    let now = db::now_ts();
    let id = match input.id {
        Some(id) => {
            let existing = db::get_entry(&conn, id).map_err(|_| "Операция не найдена".to_string())?;
            if existing.seller_id != seller.id {
                return Err("Операция относится к другому самозанятому".into());
            }
            conn.execute(
                "UPDATE entries SET date = ?1, amount_kop = ?2, payer_type = ?3, client_id = ?4, \
                 description = ?5, receipt_no = ?6, receipt_issued = ?7, updated_at = ?8 WHERE id = ?9",
                params![
                    input.date,
                    input.amount_kop,
                    input.payer_type,
                    input.client_id,
                    input.description.trim(),
                    input.receipt_no.trim(),
                    if input.receipt_issued { 1 } else { 0 },
                    now,
                    id
                ],
            )
            .map_err(to_err)?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO entries (seller_id, date, amount_kop, payer_type, client_id, description, \
                 receipt_no, receipt_issued, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                params![
                    seller.id,
                    input.date,
                    input.amount_kop,
                    input.payer_type,
                    input.client_id,
                    input.description.trim(),
                    input.receipt_no.trim(),
                    if input.receipt_issued { 1 } else { 0 },
                    now
                ],
            )
            .map_err(to_err)?;
            conn.last_insert_rowid()
        }
    };
    db::get_entry(&conn, id).map_err(to_err)
}

#[tauri::command]
pub fn delete_entry(state: State<'_, AppState>, id: i64) -> R<()> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    let removed = conn
        .execute(
            "DELETE FROM entries WHERE id = ?1 AND seller_id = ?2",
            params![id, seller.id],
        )
        .map_err(to_err)?;
    if removed == 0 {
        return Err("Операция не найдена".into());
    }
    Ok(())
}

// ---------- клиенты ----------

#[tauri::command]
pub fn list_clients(state: State<'_, AppState>, include_archived: bool) -> R<Vec<Client>> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    db::list_clients(&conn, seller.id, include_archived).map_err(to_err)
}

#[tauri::command]
pub fn save_client(state: State<'_, AppState>, input: ClientInput) -> R<Client> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Укажите имя или название клиента".into());
    }
    let inn = input.inn.trim();
    if !inn.is_empty() && !inn.chars().all(|c| c.is_ascii_digit()) {
        return Err("ИНН может содержать только цифры".into());
    }
    let kind = if input.kind == PAYER_ORGANIZATION {
        PAYER_ORGANIZATION
    } else {
        PAYER_INDIVIDUAL
    };
    let id = match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE clients SET name = ?1, inn = ?2, kind = ?3, note = ?4 WHERE id = ?5",
                params![name, inn, kind, input.note.trim(), id],
            )
            .map_err(to_err)?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO clients (name, inn, kind, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, inn, kind, input.note.trim(), db::now_ts()],
            )
            .map_err(to_err)?;
            conn.last_insert_rowid()
        }
    };
    db::get_client(&conn, seller.id, id).map_err(to_err)
}

#[tauri::command]
pub fn set_client_archived(state: State<'_, AppState>, id: i64, archived: bool) -> R<()> {
    let conn = lock(&state)?;
    conn.execute(
        "UPDATE clients SET archived = ?1 WHERE id = ?2",
        params![if archived { 1 } else { 0 }, id],
    )
    .map_err(to_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_client(state: State<'_, AppState>, id: i64) -> R<()> {
    let conn = lock(&state)?;
    let used: i64 = conn
        .query_row("SELECT COUNT(*) FROM entries WHERE client_id = ?1", params![id], |r| r.get(0))
        .map_err(to_err)?;
    if used > 0 {
        return Err("У клиента есть операции — отправьте его в архив".into());
    }
    conn.execute("DELETE FROM clients WHERE id = ?1", params![id])
        .map_err(to_err)?;
    Ok(())
}

// ---------- услуги ----------

#[tauri::command]
pub fn list_services(state: State<'_, AppState>) -> R<Vec<Service>> {
    let conn = lock(&state)?;
    db::list_services(&conn).map_err(to_err)
}

#[tauri::command]
pub fn save_service(state: State<'_, AppState>, input: ServiceInput) -> R<i64> {
    let conn = lock(&state)?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Укажите наименование услуги".into());
    }
    if input.price_kop < 0 {
        return Err("Цена не может быть отрицательной".into());
    }
    let category = input.category.trim();
    if category.chars().count() > 60 {
        return Err("Название категории — не больше 60 символов".into());
    }
    let unit = if input.unit.trim().is_empty() {
        "усл.".to_string()
    } else {
        input.unit.trim().to_string()
    };
    let tx = conn.unchecked_transaction().map_err(to_err)?;
    // Категория, введённая вручную, сразу попадает в справочник.
    let category = db::ensure_service_category(&tx, category)
        .map_err(to_err)?
        .map(|(_, name)| name)
        .unwrap_or_default();
    let id = match input.id {
        Some(id) => {
            tx.execute(
                "UPDATE services SET name = ?1, unit = ?2, price_kop = ?3, category = ?4 WHERE id = ?5",
                params![name, unit, input.price_kop, category, id],
            )
            .map_err(to_err)?;
            id
        }
        None => {
            let order: i64 = tx
                .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM services", [], |r| r.get(0))
                .map_err(to_err)?;
            tx.execute(
                "INSERT INTO services (name, unit, price_kop, category, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, unit, input.price_kop, category, order],
            )
            .map_err(to_err)?;
            tx.last_insert_rowid()
        }
    };
    tx.commit().map_err(to_err)?;
    Ok(id)
}

#[tauri::command]
pub fn delete_service(state: State<'_, AppState>, id: i64) -> R<()> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM services WHERE id = ?1", params![id])
        .map_err(to_err)?;
    Ok(())
}

// ---------- категории услуг ----------

#[tauri::command]
pub fn list_service_categories(state: State<'_, AppState>) -> R<Vec<ServiceCategory>> {
    let conn = lock(&state)?;
    db::list_service_categories(&conn).map_err(to_err)
}

#[tauri::command]
pub fn save_service_category(state: State<'_, AppState>, input: ServiceCategoryInput) -> R<i64> {
    let conn = lock(&state)?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Укажите название категории".into());
    }
    if name.chars().count() > 60 {
        return Err("Название категории — не больше 60 символов".into());
    }
    if db::category_name_taken(&conn, name, input.id).map_err(to_err)? {
        return Err("Категория с таким названием уже есть".into());
    }
    if let Some(id) = input.id {
        let exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM service_categories WHERE id = ?1", params![id], |r| {
                r.get(0)
            })
            .map_err(to_err)?;
        if exists == 0 {
            return Err("Категория не найдена".into());
        }
    }
    db::save_service_category(
        &conn,
        &ServiceCategoryInput {
            id: input.id,
            name: name.to_string(),
        },
    )
    .map_err(to_err)
}

#[tauri::command]
pub fn delete_service_category(state: State<'_, AppState>, id: i64) -> R<()> {
    let conn = lock(&state)?;
    let exists: i64 = conn
        .query_row("SELECT COUNT(*) FROM service_categories WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .map_err(to_err)?;
    if exists == 0 {
        return Err("Категория не найдена".into());
    }
    db::delete_service_category(&conn, id).map_err(to_err)
}

// ---------- импорт чека НПД ----------

#[tauri::command]
pub fn import_receipt(state: State<'_, AppState>, path: String) -> R<ReceiptImport> {
    let file_path = PathBuf::from(&path);
    let bytes = std::fs::read(&file_path).map_err(|e| format!("Не удалось открыть файл: {e}"))?;
    if bytes.len() > 25 * 1024 * 1024 {
        return Err("Файл больше 25 МБ".into());
    }
    let outcome = import::import_bytes(&bytes, &state.attachments_dir)?;
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;

    let existing = db::file_path_by_sha(&conn, &outcome.sha256).map_err(to_err)?;
    let (file_id, linked_document_id) = match existing {
        Some((id, _)) => {
            let linked = db::document_id_by_file(&conn, seller.id, id).map_err(to_err)?;
            (id, linked)
        }
        None => {
            conn.execute(
                "INSERT INTO files (sha256, rel_path, original_name, width, height, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    outcome.sha256,
                    outcome.rel_path,
                    file_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    outcome.width,
                    outcome.height,
                    db::now_ts()
                ],
            )
            .map_err(to_err)?;
            (conn.last_insert_rowid(), None)
        }
    };

    Ok(ReceiptImport {
        path,
        file_name: file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        file_id,
        sha256: outcome.sha256,
        width: outcome.width,
        height: outcome.height,
        number: outcome.number,
        inn: outcome.inn,
        payload: outcome.payload,
        preview_data_url: outcome.preview_data_url,
        duplicate_file_id: existing.map(|(id, _)| id),
        linked_document_id,
    })
}

#[tauri::command]
pub fn receipt_data_url(state: State<'_, AppState>, file_id: i64) -> R<String> {
    let conn = lock(&state)?;
    let rel: String = conn
        .query_row("SELECT rel_path FROM files WHERE id = ?1", params![file_id], |r| r.get(0))
        .map_err(|_| "Чек не найден".to_string())?;
    drop(conn);
    import::data_url_for_file(&state.attachments_dir.join(rel))
}

#[tauri::command]
pub fn receipt_thumb_data_url(state: State<'_, AppState>, file_id: i64) -> R<String> {
    let conn = lock(&state)?;
    let rel: String = conn
        .query_row("SELECT rel_path FROM files WHERE id = ?1", params![file_id], |r| r.get(0))
        .map_err(|_| "Чек не найден".to_string())?;
    drop(conn);
    import::thumbnail_data_url(&state.attachments_dir.join(rel), 140)
}

// ---------- детализация заказа ----------

fn receipt_payload(doc: &Document) -> Option<String> {
    if !doc.qr_payload.trim().is_empty() {
        return Some(doc.qr_payload.trim().to_string());
    }
    if doc.seller_inn.is_empty() || doc.number.is_empty() {
        return None;
    }
    Some(format!(
        "https://lknpd.nalog.ru/api/v1/receipt/{}/{}/print",
        doc.seller_inn, doc.number
    ))
}

fn build_document(input: &DocumentInput, settings: &Settings, seller: &Seller) -> R<Document> {
    let number = input.number.trim();
    if number.is_empty() && input.receipt_file_id.is_some() {
        return Err("Укажите номер чека НПД".into());
    }
    if !valid_date(&input.doc_date) {
        return Err("Некорректная дата документа".into());
    }
    if input.items.is_empty() {
        return Err("Добавьте хотя бы одну позицию".into());
    }
    let mut items = Vec::with_capacity(input.items.len());
    let mut total = 0i64;
    for (idx, it) in input.items.iter().enumerate() {
        let name = it.name.trim();
        if name.is_empty() {
            return Err(format!("Позиция {}: пустое наименование", idx + 1));
        }
        if it.qty_milli <= 0 {
            return Err(format!("Позиция {}: количество должно быть больше нуля", idx + 1));
        }
        if it.price_kop < 0 {
            return Err(format!("Позиция {}: цена не может быть отрицательной", idx + 1));
        }
        let sum = round_div(it.price_kop as i128 * it.qty_milli as i128, 1000);
        total += sum;
        items.push(DocumentItem {
            id: 0,
            document_id: input.id.unwrap_or(0),
            position: idx as i64,
            name: name.to_string(),
            qty_milli: it.qty_milli,
            unit: if it.unit.trim().is_empty() {
                settings.default_unit.clone()
            } else {
                it.unit.trim().to_string()
            },
            price_kop: it.price_kop,
            sum_kop: sum,
        });
    }
    if total <= 0 {
        return Err("Сумма документа должна быть больше нуля".into());
    }
    Ok(Document {
        id: input.id.unwrap_or(0),
        seller_id: seller.id,
        number: number.to_string(),
        doc_date: input.doc_date.clone(),
        buyer_name: input.buyer_name.trim().to_string(),
        buyer_inn: input.buyer_inn.trim().to_string(),
        total_kop: total,
        note: input.note.trim().to_string(),
        seller_name: input
            .seller_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| seller.name.clone()),
        seller_inn: input
            .seller_inn
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| seller.inn.clone()),
        receipt_file_id: input.receipt_file_id,
        receipt_path: None,
        receipt_pending: input.receipt_pending && input.receipt_file_id.is_none(),
        qr_payload: input.qr_payload.trim().to_string(),
        created_at: String::new(),
        items,
    })
}

fn pdf_bytes(state: &AppState, conn: &rusqlite::Connection, doc: &Document) -> R<Vec<u8>> {
    let settings = db::load_settings(conn);
    let seller = db::get_seller(conn, doc.seller_id)
        .or_else(|_| db::active_seller(conn))
        .map_err(to_err)?;
    let receipt = match doc.receipt_file_id {
        Some(file_id) => {
            let rel: String = conn
                .query_row("SELECT rel_path FROM files WHERE id = ?1", params![file_id], |r| r.get(0))
                .map_err(|_| "Чек НПД не найден в хранилище".to_string())?;
            let (w, h, rgb) = import::load_receipt(&state.attachments_dir.join(rel))?;
            Some(pdf::ReceiptImage {
                width: w,
                height: h,
                rgb,
            })
        }
        None => None,
    };
    pdf::build(pdf::BuildInput {
        doc,
        settings: &settings,
        seller: &seller,
        receipt,
        qr_payload: receipt_payload(doc),
    })
}

fn safe_filename(part: &str) -> String {
    let mut out: String = part
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '_'
            } else {
                '_'
            }
        })
        .collect();
    out = out.trim_matches('_').to_string();
    if out.chars().count() > 40 {
        out = out.chars().take(40).collect();
    }
    out
}

fn pdf_path_for(
    conn: &rusqlite::Connection,
    state: &AppState,
    settings: &Settings,
    doc: &Document,
) -> PathBuf {
    let mut root = if settings.documents_dir.trim().is_empty() {
        state.docs_dir.clone()
    } else {
        PathBuf::from(settings.documents_dir.trim())
    };
    // У профиля, перенесённого из старой версии, папка пустая — файлы лежат
    // прямо в корне, как и раньше.
    if let Ok(seller) = db::get_seller(conn, doc.seller_id) {
        if !seller.folder.trim().is_empty() {
            root = root.join(seller.folder.trim());
        }
    }
    let month_dir = if doc.doc_date.len() >= 7 {
        doc.doc_date[..7].to_string()
    } else {
        "без-даты".to_string()
    };
    let mut name = if doc.number.trim().is_empty() {
        // Без чека НПД номера ещё нет — используем дату и время создания,
        // чтобы документы за один день не перезаписывали друг друга.
        let time = doc
            .created_at
            .get(11..19)
            .map(|t| t.replace(':', ""))
            .unwrap_or_default();
        if time.is_empty() {
            format!("{}_Детализация", doc.doc_date)
        } else {
            format!("{}_Детализация_{}", doc.doc_date, time)
        }
    } else {
        format!("{}_{}", doc.doc_date, safe_filename(&doc.number))
    };
    if !doc.buyer_name.is_empty() {
        name.push('_');
        name.push_str(&safe_filename(&doc.buyer_name));
    }
    root.join(month_dir).join(format!("{name}.pdf"))
}

fn write_pdf(state: &AppState, conn: &rusqlite::Connection, doc: &Document) -> R<String> {
    let settings = db::load_settings(conn);
    let bytes = pdf_bytes(state, conn, doc)?;
    let path = pdf_path_for(conn, state, &settings, doc);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(to_err)?;
    }
    if let Err(e) = std::fs::write(&path, &bytes) {
        let locked = e.kind() == std::io::ErrorKind::PermissionDenied
            || e.raw_os_error() == Some(32)
            || e.raw_os_error() == Some(33);
        if locked {
            return Err(format!(
                "Не удалось записать PDF: файл открыт в другой программе. \
                 Закройте его и повторите сохранение.\n{}",
                path.to_string_lossy()
            ));
        }
        return Err(to_err(e));
    }
    Ok(path.to_string_lossy().to_string())
}

/// ИНН из ссылки чека ФНС: https://lknpd.nalog.ru/api/v1/receipt/<ИНН>/<номер>/print
fn qr_inn(payload: &str) -> Option<String> {
    let rest = payload.split("/receipt/").nth(1)?;
    let inn = rest.split('/').next()?;
    if inn.len() == 12 && inn.chars().all(|c| c.is_ascii_digit()) {
        Some(inn.to_string())
    } else {
        None
    }
}

#[tauri::command]
pub fn preview_document_pdf(state: State<'_, AppState>, input: DocumentInput) -> R<String> {
    let conn = lock(&state)?;
    let settings = db::load_settings(&conn);
    let seller = db::active_seller(&conn).map_err(to_err)?;
    let doc = build_document(&input, &settings, &seller)?;
    let bytes = pdf_bytes(&state, &conn, &doc)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
pub fn save_document(state: State<'_, AppState>, input: DocumentInput) -> R<serde_json::Value> {
    let conn = lock(&state)?;
    let settings = db::load_settings(&conn);
    let seller = db::active_seller(&conn).map_err(to_err)?;
    let mut doc = build_document(&input, &settings, &seller)?;
    if doc.receipt_file_id.is_none() && !doc.receipt_pending {
        return Err(
            "Прикрепите чек НПД (PNG) или отметьте, что чек не добавлен по техническим причинам"
                .into(),
        );
    }
    if doc.seller_inn.is_empty() {
        return Err("Заполните ИНН самозанятого в профиле".into());
    }
    if let Some(inn) = qr_inn(&doc.qr_payload) {
        if inn != doc.seller_inn {
            return Err(format!(
                "Чек НПД выдан самозанятому с ИНН {inn}. Переключитесь на его профиль и повторите."
            ));
        }
    }

    let previous = match input.id {
        Some(id) => {
            let old = db::get_document(&conn, id).map_err(|_| "Документ не найден".to_string())?;
            if old.seller_id != seller.id {
                return Err("Документ относится к другому самозанятому".into());
            }
            // Сохраняем дату создания: от неё зависит имя PDF у документа без номера.
            doc.created_at = old.created_at.clone();
            Some(pdf_path_for(&conn, &state, &settings, &old))
        }
        None => {
            doc.created_at = db::now_ts();
            None
        }
    };

    // PDF пишем до записи в базу: если файл открыт в просмотрщике и не
    // перезаписывается, операция не сохранится и повтор не создаст дубль.
    let pdf_path = write_pdf(&state, &conn, &doc)?;

    let now = db::now_ts();
    let id = match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE documents SET number = ?1, doc_date = ?2, buyer_name = ?3, buyer_inn = ?4, \
                 total_kop = ?5, note = ?6, seller_name = ?7, seller_inn = ?8, receipt_file_id = ?9, \
                 receipt_pending = ?10, qr_payload = ?11, updated_at = ?12 WHERE id = ?13",
                params![
                    doc.number,
                    doc.doc_date,
                    doc.buyer_name,
                    doc.buyer_inn,
                    doc.total_kop,
                    doc.note,
                    doc.seller_name,
                    doc.seller_inn,
                    doc.receipt_file_id,
                    doc.receipt_pending,
                    doc.qr_payload,
                    now,
                    id
                ],
            )
            .map_err(to_err)?;
            conn.execute("DELETE FROM document_items WHERE document_id = ?1", params![id])
                .map_err(to_err)?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO documents (seller_id, number, doc_date, buyer_name, buyer_inn, total_kop, note, \
                 seller_name, seller_inn, receipt_file_id, receipt_pending, qr_payload, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    doc.seller_id,
                    doc.number,
                    doc.doc_date,
                    doc.buyer_name,
                    doc.buyer_inn,
                    doc.total_kop,
                    doc.note,
                    doc.seller_name,
                    doc.seller_inn,
                    doc.receipt_file_id,
                    doc.receipt_pending,
                    doc.qr_payload,
                    doc.created_at,
                    now
                ],
            )
            .map_err(to_err)?;
            conn.last_insert_rowid()
        }
    };

    for item in &doc.items {
        conn.execute(
            "INSERT INTO document_items (document_id, position, name, qty_milli, unit, price_kop, sum_kop) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                item.position,
                item.name,
                item.qty_milli,
                item.unit,
                item.price_kop,
                item.sum_kop
            ],
        )
        .map_err(to_err)?;
    }

    if let Some(old) = previous {
        if old != PathBuf::from(&pdf_path) {
            let _ = std::fs::remove_file(old);
        }
    }

    Ok(serde_json::json!({ "id": id, "pdfPath": pdf_path }))
}

#[tauri::command]
pub fn list_documents(state: State<'_, AppState>, search: Option<String>) -> R<Vec<Document>> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    db::list_documents(&conn, seller.id, search.as_deref(), 500).map_err(to_err)
}

#[tauri::command]
pub fn get_document(state: State<'_, AppState>, id: i64) -> R<Document> {
    let conn = lock(&state)?;
    db::get_document(&conn, id).map_err(to_err)
}

#[tauri::command]
pub fn document_pdf(state: State<'_, AppState>, id: i64) -> R<String> {
    let conn = lock(&state)?;
    let doc = db::get_document(&conn, id).map_err(to_err)?;
    let settings = db::load_settings(&conn);
    let path = pdf_path_for(&conn, &state, &settings, &doc);
    // Уже сохранённый PDF не перезаписываем: файл может быть открыт в просмотрщике.
    let ready = std::fs::metadata(&path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false);
    if ready {
        return Ok(path.to_string_lossy().to_string());
    }
    write_pdf(&state, &conn, &doc)
}

#[tauri::command]
pub fn delete_document(state: State<'_, AppState>, id: i64) -> R<()> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    if let Ok(doc) = db::get_document(&conn, id) {
        if doc.seller_id != seller.id {
            return Err("Документ относится к другому самозанятому".into());
        }
        let settings = db::load_settings(&conn);
        let path = pdf_path_for(&conn, &state, &settings, &doc);
        let _ = std::fs::remove_file(path);
    }
    let removed = conn
        .execute(
            "DELETE FROM documents WHERE id = ?1 AND seller_id = ?2",
            params![id, seller.id],
        )
        .map_err(to_err)?;
    if removed == 0 {
        return Err("Документ не найден".into());
    }
    Ok(())
}

// ---------- отчёты и экспорт ----------

#[tauri::command]
pub fn export_csv(
    state: State<'_, AppState>,
    path: String,
    date_from: Option<String>,
    date_to: Option<String>,
) -> R<String> {
    let conn = lock(&state)?;
    let seller = db::active_seller(&conn).map_err(to_err)?;
    let entries = db::list_entries(
        &conn,
        seller.id,
        date_from.as_deref(),
        date_to.as_deref(),
        None,
        None,
        None,
    )
    .map_err(to_err)?;
    let mut out = String::from("\u{feff}Дата;Сумма;Плательщик;Клиент;Описание;Чек;Статус чека\r\n");
    for e in &entries {
        let payer = if e.payer_type == PAYER_ORGANIZATION {
            "Юрлицо"
        } else {
            "Физлицо"
        };
        out.push_str(&format!(
            "{};{};{};{};{};{};{}\r\n",
            e.date,
            pdf::format_money(e.amount_kop).replace(' ', ""),
            payer,
            escape_csv(e.client_name.as_deref().unwrap_or("")),
            escape_csv(&e.description),
            escape_csv(&e.receipt_no),
            if e.receipt_issued { "выдан" } else { "не выдан" }
        ));
    }
    std::fs::write(&path, out).map_err(to_err)?;
    Ok(path)
}

fn escape_csv(value: &str) -> String {
    if value.contains(';') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[tauri::command]
pub fn backup(state: State<'_, AppState>, target_dir: String) -> R<String> {
    let conn = lock(&state)?;
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    let stamp = Local::now().format("%Y-%m-%d_%H-%M").to_string();
    let dir = PathBuf::from(&target_dir).join(format!("cz_help_backup_{stamp}"));
    std::fs::create_dir_all(&dir).map_err(to_err)?;
    std::fs::copy(&state.db_path, dir.join("cz_help.db")).map_err(to_err)?;
    let attach_target = dir.join("attachments");
    copy_dir(&state.attachments_dir, &attach_target)?;
    let settings = db::load_settings(&conn);
    if let Ok(json) = serde_json::to_vec_pretty(&settings) {
        let _ = std::fs::write(dir.join("settings.json"), json);
    }
    if let Ok(sellers) = db::list_sellers(&conn, true) {
        if let Ok(json) = serde_json::to_vec_pretty(&sellers) {
            let _ = std::fs::write(dir.join("sellers.json"), json);
        }
    }
    Ok(dir.to_string_lossy().to_string())
}

fn copy_dir(from: &Path, to: &Path) -> R<()> {
    if !from.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(to_err)?;
    for entry in std::fs::read_dir(from).map_err(to_err)? {
        let entry = entry.map_err(to_err)?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target).map_err(to_err)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_path(path: String) -> R<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .map_err(to_err)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open").arg(&path).spawn().map_err(to_err)?;
    }
    Ok(())
}

#[tauri::command]
pub fn reveal_path(path: String) -> R<()> {
    let p = PathBuf::from(&path);
    let is_file = p.is_file();
    let target = if is_file {
        p.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| p.clone())
    } else {
        p.clone()
    };
    #[cfg(target_os = "windows")]
    {
        if is_file {
            // explorer.exe /select,"C:\path\file.pdf" — выделяет файл в проводнике.
            std::process::Command::new("explorer.exe")
                .arg(format!("/select,{}", p.display()))
                .spawn()
                .map_err(to_err)?;
        } else {
            std::process::Command::new("explorer.exe")
                .arg(target)
                .spawn()
                .map_err(to_err)?;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open").arg(target).spawn().map_err(to_err)?;
    }
    Ok(())
}
