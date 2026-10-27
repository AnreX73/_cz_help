use crate::models::*;
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(SCHEMA)?;
    migrate(&conn)?;
    seed(&conn)?;
    Ok(conn)
}

pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn now_ts() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sellers (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL DEFAULT '',
    inn        TEXT NOT NULL DEFAULT '',
    contact    TEXT NOT NULL DEFAULT '',
    city       TEXT NOT NULL DEFAULT '',
    status     TEXT NOT NULL DEFAULT 'Плательщик налога на профессиональный доход',
    folder     TEXT NOT NULL DEFAULT '',
    archived   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS clients (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    inn        TEXT NOT NULL DEFAULT '',
    kind       TEXT NOT NULL DEFAULT 'individual',
    note       TEXT NOT NULL DEFAULT '',
    archived   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS entries (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    seller_id      INTEGER NOT NULL DEFAULT 0,
    date           TEXT NOT NULL,
    amount_kop     INTEGER NOT NULL CHECK (amount_kop > 0),
    payer_type     TEXT NOT NULL DEFAULT 'individual',
    client_id      INTEGER REFERENCES clients(id) ON DELETE SET NULL,
    description    TEXT NOT NULL DEFAULT '',
    receipt_no     TEXT NOT NULL DEFAULT '',
    receipt_issued INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entries_date ON entries(date);

CREATE TABLE IF NOT EXISTS services (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    unit       TEXT NOT NULL DEFAULT 'усл.',
    price_kop  INTEGER NOT NULL DEFAULT 0,
    category   TEXT NOT NULL DEFAULT '',
    archived   INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS service_categories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS files (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    sha256        TEXT NOT NULL UNIQUE,
    rel_path      TEXT NOT NULL,
    original_name TEXT NOT NULL,
    width         INTEGER NOT NULL DEFAULT 0,
    height        INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS documents (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    seller_id       INTEGER NOT NULL DEFAULT 0,
    number          TEXT NOT NULL,
    doc_date        TEXT NOT NULL,
    buyer_name      TEXT NOT NULL DEFAULT '',
    buyer_inn       TEXT NOT NULL DEFAULT '',
    total_kop       INTEGER NOT NULL DEFAULT 0,
    note            TEXT NOT NULL DEFAULT '',
    seller_name     TEXT NOT NULL DEFAULT '',
    seller_inn      TEXT NOT NULL DEFAULT '',
    receipt_file_id INTEGER REFERENCES files(id) ON DELETE SET NULL,
    receipt_pending INTEGER NOT NULL DEFAULT 0,
    qr_payload      TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_documents_date ON documents(doc_date);
CREATE INDEX IF NOT EXISTS idx_documents_number ON documents(number);

CREATE TABLE IF NOT EXISTS document_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    qty_milli   INTEGER NOT NULL DEFAULT 1000,
    unit        TEXT NOT NULL DEFAULT 'шт',
    price_kop   INTEGER NOT NULL DEFAULT 0,
    sum_kop     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_document_items_doc ON document_items(document_id);

CREATE TABLE IF NOT EXISTS payments (
    seller_id  INTEGER NOT NULL,
    year       INTEGER NOT NULL,
    month      INTEGER NOT NULL,
    amount_kop INTEGER NOT NULL DEFAULT 0,
    paid_at    TEXT NOT NULL,
    PRIMARY KEY (seller_id, year, month)
);
"#;

const DEFAULT_SELLER_STATUS: &str = "Плательщик налога на профессиональный доход";

/// Приводит базу к актуальной схеме. Работает и со старой базой, где продавец
/// был один и хранился в настройках, и со свежей.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let sellers_count: i64 = conn.query_row("SELECT COUNT(*) FROM sellers", [], |r| r.get(0))?;
    if sellers_count == 0 {
        let legacy = legacy_seller(conn);
        let id = insert_seller(conn, &legacy, "", false)?;
        set_meta(conn, "active_seller", &id.to_string())?;
    }

    if !has_column(conn, "entries", "seller_id")? {
        conn.execute_batch("ALTER TABLE entries ADD COLUMN seller_id INTEGER NOT NULL DEFAULT 0")?;
    }
    if !has_column(conn, "documents", "seller_id")? {
        conn.execute_batch("ALTER TABLE documents ADD COLUMN seller_id INTEGER NOT NULL DEFAULT 0")?;
    }
    if !has_column(conn, "documents", "receipt_pending")? {
        conn.execute_batch(
            "ALTER TABLE documents ADD COLUMN receipt_pending INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_entries_seller_date ON entries(seller_id, date);
         CREATE INDEX IF NOT EXISTS idx_documents_seller_date ON documents(seller_id, doc_date);",
    )?;

    let active_id = active_seller_id(conn)?;
    conn.execute("UPDATE entries SET seller_id = ?1 WHERE seller_id = 0", params![active_id])?;
    conn.execute("UPDATE documents SET seller_id = ?1 WHERE seller_id = 0", params![active_id])?;

    if !has_column(conn, "payments", "seller_id")? {
        conn.execute_batch(&format!(
            "ALTER TABLE payments RENAME TO payments_old;
             CREATE TABLE payments (
                 seller_id  INTEGER NOT NULL,
                 year       INTEGER NOT NULL,
                 month      INTEGER NOT NULL,
                 amount_kop INTEGER NOT NULL DEFAULT 0,
                 paid_at    TEXT NOT NULL,
                 PRIMARY KEY (seller_id, year, month)
             );
             INSERT INTO payments (seller_id, year, month, amount_kop, paid_at)
                 SELECT {active_id}, year, month, amount_kop, paid_at FROM payments_old;
             DROP TABLE payments_old;"
        ))?;
    }

    // Активный продавец мог быть удалён или отправлен в архив.
    let still_active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sellers WHERE id = ?1 AND archived = 0",
        params![active_id],
        |r| r.get(0),
    )?;
    if still_active == 0 {
        let next: Option<i64> = conn
            .query_row(
                "SELECT id FROM sellers WHERE archived = 0 ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = next {
            set_meta(conn, "active_seller", &id.to_string())?;
        }
    }

    // До появления справочника категории жили только в services.category.
    backfill_service_categories(conn)?;

    Ok(())
}

/// Добавляет в справочник категории, которые встречаются только в услугах.
/// Названия, отличающиеся регистром, считаются одной категорией.
fn backfill_service_categories(conn: &Connection) -> rusqlite::Result<()> {
    let mut existing: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut next_order: i64 = 0;
    {
        let mut stmt = conn.prepare("SELECT name, sort_order FROM service_categories")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let order: i64 = row.get(1)?;
            next_order = next_order.max(order + 1);
            existing.entry(name.to_lowercase()).or_insert(name);
        }
    }

    let mut pending: Vec<(i64, String)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, TRIM(category) FROM services \
             WHERE TRIM(category) <> '' ORDER BY sort_order, id",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            pending.push((row.get(0)?, row.get(1)?));
        }
    }

    for (service_id, name) in pending {
        let key = name.to_lowercase();
        let canonical = match existing.get(&key) {
            Some(saved) => saved.clone(),
            None => {
                conn.execute(
                    "INSERT INTO service_categories (name, sort_order) VALUES (?1, ?2)",
                    params![name, next_order],
                )?;
                next_order += 1;
                existing.insert(key, name.clone());
                name.clone()
            }
        };
        if canonical != name {
            conn.execute(
                "UPDATE services SET category = ?1 WHERE id = ?2",
                params![canonical, service_id],
            )?;
        }
    }
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn legacy_seller(conn: &Connection) -> SellerInput {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'settings'", [], |r| r.get(0))
        .optional()
        .ok()
        .flatten();
    let json: serde_json::Value = raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);
    let get = |key: &str| {
        json.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let status = get("sellerStatus");
    SellerInput {
        id: None,
        name: get("sellerName"),
        inn: get("sellerInn"),
        contact: get("sellerContact"),
        city: get("sellerCity"),
        status: if status.is_empty() {
            DEFAULT_SELLER_STATUS.to_string()
        } else {
            status
        },
    }
}

fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let seeded: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'seeded'", [], |r| r.get(0))
        .optional()?;
    if seeded.is_some() {
        return Ok(());
    }
    let templates: [(&str, &str, i64, &str); 9] = [
        ("Фото на документы 3×4, 4 шт.", "усл.", 400_00, "Фотоуслуги"),
        ("Фото на документы 3×4, 6 шт.", "усл.", 500_00, "Фотоуслуги"),
        ("Фото на документы 3×4, 8 шт.", "усл.", 600_00, "Фотоуслуги"),
        ("Печать фотографий 10×15", "шт.", 35_00, "Фотопечать"),
        ("Печать фотографий A4", "шт.", 150_00, "Фотопечать"),
        ("Реставрация фотографии", "усл.", 500_00, "Обработка"),
        ("Сканирование документа", "шт.", 20_00, "Полиграфия"),
        ("Ламинирование A4", "шт.", 80_00, "Полиграфия"),
        ("Копирование документа A4", "шт.", 10_00, "Полиграфия"),
    ];
    let tx = conn.unchecked_transaction()?;
    for (i, (name, unit, price, category)) in templates.iter().enumerate() {
        tx.execute(
            "INSERT INTO services (name, unit, price_kop, category, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, unit, price, category, i as i64],
        )?;
    }
    tx.execute("INSERT INTO meta (key, value) VALUES ('seeded', '1')", [])?;
    tx.commit()?;
    backfill_service_categories(conn)
}

pub fn load_settings(conn: &Connection) -> Settings {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'settings'", [], |r| r.get(0))
        .optional()
        .ok()
        .flatten();
    match raw.and_then(|json| serde_json::from_str::<Settings>(&json).ok()) {
        Some(mut s) => {
            let d = Settings::default();
            if s.rate_individual <= 0 || s.rate_individual > 1000 {
                s.rate_individual = d.rate_individual;
            }
            if s.rate_organization <= 0 || s.rate_organization > 1000 {
                s.rate_organization = d.rate_organization;
            }
            if s.deduction_kop < 0 {
                s.deduction_kop = d.deduction_kop;
            }
            if s.limit_kop <= 0 {
                s.limit_kop = d.limit_kop;
            }
            if s.receipt_width_mm < 30 || s.receipt_width_mm > 120 {
                s.receipt_width_mm = d.receipt_width_mm;
            }
            if s.theme.is_empty() {
                s.theme = d.theme;
            }
            if s.default_unit.is_empty() {
                s.default_unit = d.default_unit;
            }
            s
        }
        None => Settings::default(),
    }
}

pub fn save_settings(conn: &Connection, s: &Settings) -> rusqlite::Result<()> {
    let json = serde_json::to_string(s).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('settings', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    )?;
    Ok(())
}

fn seller_from_row(row: &Row) -> rusqlite::Result<Seller> {
    Ok(Seller {
        id: row.get(0)?,
        name: row.get(1)?,
        inn: row.get(2)?,
        contact: row.get(3)?,
        city: row.get(4)?,
        status: row.get(5)?,
        folder: row.get(6)?,
        archived: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
    })
}

const SELLER_SELECT: &str =
    "SELECT id, name, inn, contact, city, status, folder, archived, created_at FROM sellers";

pub fn list_sellers(conn: &Connection, include_archived: bool) -> rusqlite::Result<Vec<Seller>> {
    let mut stmt = conn.prepare(&format!(
        "{SELLER_SELECT} WHERE (?1 = 1 OR archived = 0) ORDER BY archived, id"
    ))?;
    let rows = stmt.query_map(params![if include_archived { 1 } else { 0 }], seller_from_row)?;
    rows.collect()
}

pub fn get_seller(conn: &Connection, id: i64) -> rusqlite::Result<Seller> {
    conn.query_row(&format!("{SELLER_SELECT} WHERE id = ?1"), params![id], seller_from_row)
}

fn insert_seller(
    conn: &Connection,
    input: &SellerInput,
    folder: &str,
    archived: bool,
) -> rusqlite::Result<i64> {
    let status = if input.status.trim().is_empty() {
        DEFAULT_SELLER_STATUS.to_string()
    } else {
        input.status.trim().to_string()
    };
    conn.execute(
        "INSERT INTO sellers (name, inn, contact, city, status, folder, archived, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            input.name.trim(),
            input.inn.trim(),
            input.contact.trim(),
            input.city.trim(),
            status,
            folder,
            if archived { 1 } else { 0 },
            now_ts()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Папка для PDF: читаемое имя + id, чтобы совпадения имён не мешали друг другу.
fn seller_folder(name: &str, inn: &str, id: i64) -> String {
    let base = if name.trim().is_empty() { inn } else { name };
    let mut slug: String = base
        .trim()
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
    slug = slug.trim_matches('_').to_string();
    if slug.chars().count() > 36 {
        slug = slug.chars().take(36).collect();
    }
    if slug.is_empty() {
        format!("seller-{id}")
    } else {
        format!("{slug}-{id}")
    }
}

pub fn save_seller(conn: &Connection, input: &SellerInput) -> rusqlite::Result<Seller> {    let id = match input.id {
        Some(id) => {
            let status = if input.status.trim().is_empty() {
                DEFAULT_SELLER_STATUS.to_string()
            } else {
                input.status.trim().to_string()
            };
            conn.execute(
                "UPDATE sellers SET name = ?1, inn = ?2, contact = ?3, city = ?4, status = ?5 \
                 WHERE id = ?6",
                params![
                    input.name.trim(),
                    input.inn.trim(),
                    input.contact.trim(),
                    input.city.trim(),
                    status,
                    id
                ],
            )?;
            id
        }
        None => {
            let id = insert_seller(conn, input, "", false)?;
            let folder = seller_folder(input.name.trim(), input.inn.trim(), id);
            conn.execute("UPDATE sellers SET folder = ?1 WHERE id = ?2", params![folder, id])?;
            id
        }
    };
    get_seller(conn, id)
}

pub fn set_seller_archived(conn: &Connection, id: i64, archived: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sellers SET archived = ?1 WHERE id = ?2",
        params![if archived { 1 } else { 0 }, id],
    )?;
    Ok(())
}

pub fn delete_seller(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM payments WHERE seller_id = ?1", params![id])?;
    conn.execute("DELETE FROM sellers WHERE id = ?1", params![id])?;
    Ok(())
}

/// Заполняет папку у профиля-заготовки, когда в него вписали реальные данные.
/// Профили с уже сохранёнными документами не трогаем: их PDF лежат в корне.
pub fn ensure_seller_folder(conn: &Connection, id: i64) -> rusqlite::Result<Seller> {
    let seller = get_seller(conn, id)?;
    if !seller.folder.is_empty() {
        return Ok(seller);
    }
    let documents: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE seller_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if documents > 0 {
        return Ok(seller);
    }
    let folder = seller_folder(&seller.name, &seller.inn, seller.id);
    conn.execute("UPDATE sellers SET folder = ?1 WHERE id = ?2", params![folder, id])?;
    get_seller(conn, id)
}

pub fn set_active_seller(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    set_meta(conn, "active_seller", &id.to_string())
}

pub fn active_seller_id(conn: &Connection) -> rusqlite::Result<i64> {
    let stored: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'active_seller'", [], |r| r.get(0))
        .optional()?;
    if let Some(id) = stored.and_then(|v| v.parse::<i64>().ok()) {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sellers WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        if exists > 0 {
            return Ok(id);
        }
    }
    let next: Option<i64> = conn
        .query_row("SELECT id FROM sellers WHERE archived = 0 ORDER BY id LIMIT 1", [], |r| r.get(0))
        .optional()?;
    if let Some(id) = next {
        set_meta(conn, "active_seller", &id.to_string())?;
        return Ok(id);
    }
    let any: i64 = conn.query_row("SELECT id FROM sellers ORDER BY id LIMIT 1", [], |r| r.get(0))?;
    set_meta(conn, "active_seller", &any.to_string())?;
    Ok(any)
}

pub fn active_seller(conn: &Connection) -> rusqlite::Result<Seller> {
    get_seller(conn, active_seller_id(conn)?)
}

fn entry_from_row(row: &Row) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: row.get(0)?,
        seller_id: row.get(1)?,
        date: row.get(2)?,
        amount_kop: row.get(3)?,
        payer_type: row.get(4)?,
        client_id: row.get(5)?,
        client_name: row.get(6)?,
        description: row.get(7)?,
        receipt_no: row.get(8)?,
        receipt_issued: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
    })
}

const ENTRY_SELECT: &str = "SELECT e.id, e.seller_id, e.date, e.amount_kop, e.payer_type, e.client_id, \
     c.name, e.description, e.receipt_no, e.receipt_issued, e.created_at \
     FROM entries e LEFT JOIN clients c ON c.id = e.client_id";

pub fn list_entries(
    conn: &Connection,
    seller_id: i64,
    date_from: Option<&str>,
    date_to: Option<&str>,
    payer_type: Option<&str>,
    search: Option<&str>,
    limit: Option<i64>,
) -> rusqlite::Result<Vec<Entry>> {
    let mut sql = String::from(ENTRY_SELECT);
    let mut where_parts: Vec<String> = vec!["e.seller_id = ?".into()];
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    args.push(Box::new(seller_id));

    if let Some(from) = date_from {
        where_parts.push("e.date >= ?".into());
        args.push(Box::new(from.to_string()));
    }
    if let Some(to) = date_to {
        where_parts.push("e.date <= ?".into());
        args.push(Box::new(to.to_string()));
    }
    if let Some(pt) = payer_type {
        if !pt.is_empty() && pt != "all" {
            where_parts.push("e.payer_type = ?".into());
            args.push(Box::new(pt.to_string()));
        }
    }
    if let Some(s) = search {
        let s = s.trim();
        if !s.is_empty() {
            where_parts.push("(e.description LIKE ? OR e.receipt_no LIKE ? OR c.name LIKE ?)".into());
            let like = format!("%{s}%");
            args.push(Box::new(like.clone()));
            args.push(Box::new(like.clone()));
            args.push(Box::new(like));
        }
    }
    if !where_parts.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&where_parts.join(" AND "));
    }
    sql.push_str(" ORDER BY e.date DESC, e.id DESC");
    if let Some(l) = limit {
        sql.push_str(&format!(" LIMIT {l}"));
    }

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(params_ref.as_slice(), entry_from_row)?;
    rows.collect()
}

pub fn get_entry(conn: &Connection, id: i64) -> rusqlite::Result<Entry> {
    conn.query_row(&format!("{ENTRY_SELECT} WHERE e.id = ?1"), params![id], entry_from_row)
}

pub fn all_entries_lite(conn: &Connection, seller_id: i64) -> rusqlite::Result<Vec<crate::tax::EntryLite>> {
    let mut stmt = conn.prepare(
        "SELECT date, amount_kop, payer_type FROM entries WHERE seller_id = ?1 ORDER BY date, id",
    )?;
    let rows = stmt.query_map(params![seller_id], |row| {
        Ok(crate::tax::EntryLite {
            date: row.get(0)?,
            amount_kop: row.get(1)?,
            payer_type: row.get(2)?,
        })
    })?;
    rows.collect()
}

fn client_from_row(row: &Row) -> rusqlite::Result<Client> {
    Ok(Client {
        id: row.get(0)?,
        name: row.get(1)?,
        inn: row.get(2)?,
        kind: row.get(3)?,
        note: row.get(4)?,
        archived: row.get::<_, i64>(5)? != 0,
        total_kop: row.get(6)?,
        entries_count: row.get(7)?,
    })
}

pub fn list_clients(conn: &Connection, seller_id: i64, include_archived: bool) -> rusqlite::Result<Vec<Client>> {
    let sql = "SELECT c.id, c.name, c.inn, c.kind, c.note, c.archived, \
        COALESCE(SUM(e.amount_kop), 0) AS total, COUNT(e.id) AS cnt \
        FROM clients c LEFT JOIN entries e ON e.client_id = c.id AND e.seller_id = ?1 \
        WHERE (?2 = 1 OR c.archived = 0) \
        GROUP BY c.id ORDER BY c.archived, c.name COLLATE NOCASE";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(
        params![seller_id, if include_archived { 1 } else { 0 }],
        client_from_row,
    )?;
    rows.collect()
}

pub fn get_client(conn: &Connection, seller_id: i64, id: i64) -> rusqlite::Result<Client> {
    conn.query_row(
        "SELECT c.id, c.name, c.inn, c.kind, c.note, c.archived, \
         COALESCE((SELECT SUM(amount_kop) FROM entries WHERE client_id = c.id AND seller_id = ?1), 0), \
         (SELECT COUNT(*) FROM entries WHERE client_id = c.id AND seller_id = ?1) \
         FROM clients c WHERE c.id = ?2",
        params![seller_id, id],
        client_from_row,
    )
}

fn service_from_row(row: &Row) -> rusqlite::Result<Service> {
    Ok(Service {
        id: row.get(0)?,
        name: row.get(1)?,
        unit: row.get(2)?,
        price_kop: row.get(3)?,
        category: row.get(4)?,
        archived: row.get::<_, i64>(5)? != 0,
    })
}

pub fn list_services(conn: &Connection) -> rusqlite::Result<Vec<Service>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, unit, price_kop, category, archived FROM services \
         WHERE archived = 0 ORDER BY sort_order, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], service_from_row)?;
    rows.collect()
}

fn category_from_row(row: &Row) -> rusqlite::Result<ServiceCategory> {
    Ok(ServiceCategory {
        id: row.get(0)?,
        name: row.get(1)?,
        service_count: row.get(2)?,
    })
}

pub fn list_service_categories(conn: &Connection) -> rusqlite::Result<Vec<ServiceCategory>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, \
         (SELECT COUNT(*) FROM services s WHERE s.archived = 0 AND TRIM(s.category) = c.name) \
         FROM service_categories c ORDER BY c.sort_order, c.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], category_from_row)?;
    rows.collect()
}

/// Ищет категорию по названию без учёта регистра (в том числе для кириллицы).
pub fn find_service_category(
    conn: &Connection,
    name: &str,
) -> rusqlite::Result<Option<(i64, String)>> {
    let target = name.trim().to_lowercase();
    if target.is_empty() {
        return Ok(None);
    }
    let mut stmt =
        conn.prepare("SELECT id, name FROM service_categories ORDER BY sort_order, name COLLATE NOCASE")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let stored: String = row.get(1)?;
        if stored.to_lowercase() == target {
            return Ok(Some((id, stored)));
        }
    }
    Ok(None)
}

pub fn category_name_taken(
    conn: &Connection,
    name: &str,
    except_id: Option<i64>,
) -> rusqlite::Result<bool> {
    let target = name.trim().to_lowercase();
    if target.is_empty() {
        return Ok(false);
    }
    let mut stmt = conn.prepare("SELECT id, name FROM service_categories")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let stored: String = row.get(1)?;
        if stored.to_lowercase() == target && Some(id) != except_id {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Создаёт категорию, если её ещё нет, и возвращает её id и каноническое название.
pub fn ensure_service_category(
    conn: &Connection,
    name: &str,
) -> rusqlite::Result<Option<(i64, String)>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if let Some(found) = find_service_category(conn, name)? {
        return Ok(Some(found));
    }
    let order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM service_categories",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO service_categories (name, sort_order) VALUES (?1, ?2)",
        params![name, order],
    )?;
    Ok(Some((conn.last_insert_rowid(), name.to_string())))
}

fn service_ids_with_category(conn: &Connection, name: &str) -> rusqlite::Result<Vec<i64>> {
    let target = name.trim().to_lowercase();
    let mut stmt =
        conn.prepare("SELECT id, TRIM(category) FROM services WHERE TRIM(category) <> ''")?;
    let mut rows = stmt.query([])?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let value: String = row.get(1)?;
        if value.to_lowercase() == target {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Создаёт или переименовывает категорию. При переименовании услуги,
/// которые ссылались на старое имя, получают новое.
pub fn save_service_category(
    conn: &Connection,
    input: &ServiceCategoryInput,
) -> rusqlite::Result<i64> {
    let name = input.name.trim();
    match input.id {
        Some(id) => {
            let old: String = conn.query_row(
                "SELECT name FROM service_categories WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )?;
            let service_ids = service_ids_with_category(conn, &old)?;
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "UPDATE service_categories SET name = ?1 WHERE id = ?2",
                params![name, id],
            )?;
            for service_id in service_ids {
                tx.execute(
                    "UPDATE services SET category = ?1 WHERE id = ?2",
                    params![name, service_id],
                )?;
            }
            tx.commit()?;
            Ok(id)
        }
        None => {
            let order: i64 = conn.query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM service_categories",
                [],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT INTO service_categories (name, sort_order) VALUES (?1, ?2)",
                params![name, order],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// Удаляет категорию, оставляя её услуги в каталоге без категории.
pub fn delete_service_category(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let name: String = conn.query_row(
        "SELECT name FROM service_categories WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    let service_ids = service_ids_with_category(conn, &name)?;
    let tx = conn.unchecked_transaction()?;
    for service_id in service_ids {
        tx.execute(
            "UPDATE services SET category = '' WHERE id = ?1",
            params![service_id],
        )?;
    }
    tx.execute("DELETE FROM service_categories WHERE id = ?1", params![id])?;
    tx.commit()
}

fn item_from_row(row: &Row) -> rusqlite::Result<DocumentItem> {
    Ok(DocumentItem {
        id: row.get(0)?,
        document_id: row.get(1)?,
        position: row.get(2)?,
        name: row.get(3)?,
        qty_milli: row.get(4)?,
        unit: row.get(5)?,
        price_kop: row.get(6)?,
        sum_kop: row.get(7)?,
    })
}

pub fn document_items(conn: &Connection, document_id: i64) -> rusqlite::Result<Vec<DocumentItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, position, name, qty_milli, unit, price_kop, sum_kop \
         FROM document_items WHERE document_id = ?1 ORDER BY position, id",
    )?;
    let rows = stmt.query_map(params![document_id], item_from_row)?;
    rows.collect()
}

pub fn get_document(conn: &Connection, id: i64) -> rusqlite::Result<Document> {
    let mut doc = conn.query_row(
        "SELECT d.id, d.seller_id, d.number, d.doc_date, d.buyer_name, d.buyer_inn, d.total_kop, d.note, \
         d.seller_name, d.seller_inn, d.receipt_file_id, d.receipt_pending, d.qr_payload, f.rel_path, d.created_at \
         FROM documents d LEFT JOIN files f ON f.id = d.receipt_file_id WHERE d.id = ?1",
        params![id],
        |row| {
            Ok(Document {
                id: row.get(0)?,
                seller_id: row.get(1)?,
                number: row.get(2)?,
                doc_date: row.get(3)?,
                buyer_name: row.get(4)?,
                buyer_inn: row.get(5)?,
                total_kop: row.get(6)?,
                note: row.get(7)?,
                seller_name: row.get(8)?,
                seller_inn: row.get(9)?,
                receipt_file_id: row.get(10)?,
                receipt_pending: row.get(11)?,
                qr_payload: row.get(12)?,
                receipt_path: row.get(13)?,
                created_at: row.get(14)?,
                items: Vec::new(),
            })
        },
    )?;
    doc.items = document_items(conn, doc.id)?;
    Ok(doc)
}

pub fn list_documents(
    conn: &Connection,
    seller_id: i64,
    search: Option<&str>,
    limit: i64,
) -> rusqlite::Result<Vec<Document>> {
    let like = format!("%{}%", search.unwrap_or("").trim());
    let mut stmt = conn.prepare(
        "SELECT d.id, d.seller_id, d.number, d.doc_date, d.buyer_name, d.buyer_inn, d.total_kop, d.note, \
         d.seller_name, d.seller_inn, d.receipt_file_id, d.receipt_pending, d.qr_payload, f.rel_path, d.created_at \
         FROM documents d LEFT JOIN files f ON f.id = d.receipt_file_id \
         WHERE d.seller_id = ?1 AND (?2 = '%%' OR d.number LIKE ?2 OR d.buyer_name LIKE ?2) \
         ORDER BY d.doc_date DESC, d.id DESC LIMIT ?3",
    )?;
    let mut docs: Vec<Document> = stmt
        .query_map(params![seller_id, like, limit], |row| {
            Ok(Document {
                id: row.get(0)?,
                seller_id: row.get(1)?,
                number: row.get(2)?,
                doc_date: row.get(3)?,
                buyer_name: row.get(4)?,
                buyer_inn: row.get(5)?,
                total_kop: row.get(6)?,
                note: row.get(7)?,
                seller_name: row.get(8)?,
                seller_inn: row.get(9)?,
                receipt_file_id: row.get(10)?,
                receipt_pending: row.get(11)?,
                qr_payload: row.get(12)?,
                receipt_path: row.get(13)?,
                created_at: row.get(14)?,
                items: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for doc in docs.iter_mut() {
        doc.items = document_items(conn, doc.id)?;
    }
    Ok(docs)
}

pub fn file_path_by_sha(conn: &Connection, sha: &str) -> rusqlite::Result<Option<(i64, String)>> {
    conn.query_row(
        "SELECT id, rel_path FROM files WHERE sha256 = ?1",
        params![sha],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

pub fn document_id_by_file(conn: &Connection, seller_id: i64, file_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM documents WHERE receipt_file_id = ?1 AND seller_id = ?2 ORDER BY id DESC LIMIT 1",
        params![file_id, seller_id],
        |r| r.get(0),
    )
    .optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tax::{self, TaxRules};

    /// Схема версии «до профилей»: продавец один и живёт в настройках.
    const LEGACY_SCHEMA: &str = r#"
    CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE clients (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        inn TEXT NOT NULL DEFAULT '',
        kind TEXT NOT NULL DEFAULT 'individual',
        note TEXT NOT NULL DEFAULT '',
        archived INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL
    );
    CREATE TABLE entries (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        date TEXT NOT NULL,
        amount_kop INTEGER NOT NULL CHECK (amount_kop > 0),
        payer_type TEXT NOT NULL DEFAULT 'individual',
        client_id INTEGER REFERENCES clients(id) ON DELETE SET NULL,
        description TEXT NOT NULL DEFAULT '',
        receipt_no TEXT NOT NULL DEFAULT '',
        receipt_issued INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE INDEX idx_entries_date ON entries(date);
    CREATE TABLE services (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        unit TEXT NOT NULL DEFAULT 'усл.',
        price_kop INTEGER NOT NULL DEFAULT 0,
        category TEXT NOT NULL DEFAULT '',
        archived INTEGER NOT NULL DEFAULT 0,
        sort_order INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE files (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        sha256 TEXT NOT NULL UNIQUE,
        rel_path TEXT NOT NULL,
        original_name TEXT NOT NULL,
        width INTEGER NOT NULL DEFAULT 0,
        height INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL
    );
    CREATE TABLE documents (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        number TEXT NOT NULL,
        doc_date TEXT NOT NULL,
        buyer_name TEXT NOT NULL DEFAULT '',
        buyer_inn TEXT NOT NULL DEFAULT '',
        total_kop INTEGER NOT NULL DEFAULT 0,
        note TEXT NOT NULL DEFAULT '',
        seller_name TEXT NOT NULL DEFAULT '',
        seller_inn TEXT NOT NULL DEFAULT '',
        receipt_file_id INTEGER REFERENCES files(id) ON DELETE SET NULL,
        qr_payload TEXT NOT NULL DEFAULT '',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE TABLE document_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        name TEXT NOT NULL,
        qty_milli INTEGER NOT NULL DEFAULT 1000,
        unit TEXT NOT NULL DEFAULT 'шт',
        price_kop INTEGER NOT NULL DEFAULT 0,
        sum_kop INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE payments (
        year INTEGER NOT NULL,
        month INTEGER NOT NULL,
        amount_kop INTEGER NOT NULL DEFAULT 0,
        paid_at TEXT NOT NULL,
        PRIMARY KEY (year, month)
    );
    "#;

    fn seed_legacy(conn: &Connection) {
        conn.execute_batch(LEGACY_SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('settings', ?1)",
            params![r#"{"sellerName":"Иванов Иван Иванович","sellerInn":"540230136541","sellerStatus":"Плательщик НПД","sellerCity":"Новосибирск","sellerContact":"ivanov@example.ru","rateIndividual":400,"rateOrganization":600,"deductionKop":1000000,"limitKop":240000000,"receiptWidthMm":70,"extraQr":true,"theme":"light","defaultUnit":"усл.","documentsDir":""}"#],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clients (name, inn, kind, note, archived, created_at) VALUES ('ООО «Ромашка»', '7701234567', 'organization', '', 0, '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entries (date, amount_kop, payer_type, client_id, description, receipt_no, receipt_issued, created_at, updated_at) \
             VALUES ('2026-03-10', 500000, 'organization', 1, 'Услуга', '', 0, '2026-03-10', '2026-03-10')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (number, doc_date, buyer_name, buyer_inn, total_kop, note, seller_name, seller_inn, receipt_file_id, qr_payload, created_at, updated_at) \
             VALUES ('abc123', '2026-03-10', 'ООО «Ромашка»', '7701234567', 500000, '', 'Иванов Иван Иванович', '540230136541', NULL, '', '2026-03-10', '2026-03-10')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payments (year, month, amount_kop, paid_at) VALUES (2026, 2, 0, '2026-03-20')",
            [],
        )
        .unwrap();
    }

    fn migrate_legacy(conn: &Connection) {
        conn.execute_batch(SCHEMA).unwrap();
        migrate(conn).unwrap();
        seed(conn).unwrap();
    }

    #[test]
    fn migrates_legacy_database_to_first_seller() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy(&conn);
        migrate_legacy(&conn);

        let sellers = list_sellers(&conn, true).unwrap();
        assert_eq!(sellers.len(), 1, "старый профиль должен стать единственным");
        let seller = &sellers[0];
        assert_eq!(seller.name, "Иванов Иван Иванович");
        assert_eq!(seller.inn, "540230136541");
        assert_eq!(seller.city, "Новосибирск");
        assert_eq!(seller.status, "Плательщик НПД");
        // У мигрированного профиля файлы остаются в корне каталога «Чеки».
        assert!(seller.folder.is_empty());

        assert_eq!(active_seller(&conn).unwrap().id, seller.id);

        let entries = list_entries(&conn, seller.id, None, None, None, None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].seller_id, seller.id);

        let docs = list_documents(&conn, seller.id, None, 10).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].seller_id, seller.id);
        assert!(!docs[0].receipt_pending, "у старых документов отметка не должна появляться");

        let paid: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM payments WHERE seller_id = ?1 AND year = 2026 AND month = 2",
                params![seller.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(paid, 1);
    }

    #[test]
    fn legacy_migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy(&conn);
        migrate_legacy(&conn);
        migrate(&conn).unwrap();
        assert_eq!(list_sellers(&conn, true).unwrap().len(), 1);
    }

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        migrate(&conn).unwrap();
        seed(&conn).unwrap();
        conn
    }

    fn add_entry(conn: &Connection, seller_id: i64, date: &str, amount: i64, client: Option<i64>) {
        conn.execute(
            "INSERT INTO entries (seller_id, date, amount_kop, payer_type, client_id, description, receipt_no, receipt_issued, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'organization', ?4, '', '', 0, ?2, ?2)",
            params![seller_id, date, amount, client],
        )
        .unwrap();
    }

    #[test]
    fn profiles_keep_income_tax_and_payments_apart() {
        let conn = fresh();
        let first = save_seller(
            &conn,
            &SellerInput {
                id: Some(active_seller_id(&conn).unwrap()),
                name: "Иванов Иван Иванович".into(),
                inn: "540230136541".into(),
                contact: String::new(),
                city: "Новосибирск".into(),
                status: String::new(),
            },
        )
        .unwrap();
        let second = save_seller(
            &conn,
            &SellerInput {
                id: None,
                name: "Петров Пётр Петрович".into(),
                inn: "540230136542".into(),
                contact: String::new(),
                city: String::new(),
                status: String::new(),
            },
        )
        .unwrap();
        assert_eq!(active_seller_id(&conn).unwrap(), first.id);
        assert!(!second.folder.is_empty());

        conn.execute(
            "INSERT INTO clients (name, inn, kind, note, archived, created_at) VALUES ('ООО «Ромашка»', '', 'organization', '', 0, '2026-01-01')",
            [],
        )
        .unwrap();
        let client: i64 = conn.last_insert_rowid();

        add_entry(&conn, first.id, "2026-01-15", 1_000_000, Some(client));
        add_entry(&conn, first.id, "2026-01-20", 2_000_000, None);
        add_entry(&conn, second.id, "2026-01-15", 5_000_000, Some(client));

        let rules = TaxRules {
            rate_individual: 400,
            rate_organization: 600,
            deduction_kop: 1_000_000,
        };
        let first_entries = all_entries_lite(&conn, first.id).unwrap();
        let second_entries = all_entries_lite(&conn, second.id).unwrap();
        assert_eq!(first_entries.len(), 2);
        assert_eq!(second_entries.len(), 1);

        let first_months = tax::compute(&first_entries, &rules);
        let second_months = tax::compute(&second_entries, &rules);
        assert_eq!(first_months.len(), 1);
        assert_eq!(first_months[0].income_total_kop, 3_000_000);
        assert_eq!(second_months[0].income_total_kop, 5_000_000);
        // Вычет у каждого свой: второй профиль не «съедает» вычет первого.
        assert_eq!(first_months[0].deduction_used_kop, 60_000);
        assert_eq!(second_months[0].deduction_used_kop, 100_000);

        // Клиенты общие, но сумма в карточке — по активному самозанятому.
        let first_clients = list_clients(&conn, first.id, false).unwrap();
        let second_clients = list_clients(&conn, second.id, false).unwrap();
        assert_eq!(first_clients.len(), 1);
        assert_eq!(first_clients[0].total_kop, 1_000_000);
        assert_eq!(second_clients[0].total_kop, 5_000_000);

        // Отметки об уплате раздельные, одинаковый месяц не конфликтует.
        conn.execute(
            "INSERT INTO payments (seller_id, year, month, amount_kop, paid_at) VALUES (?1, 2026, 1, 0, '2026-02-20')",
            params![first.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payments (seller_id, year, month, amount_kop, paid_at) VALUES (?1, 2026, 1, 0, '2026-02-20')",
            params![second.id],
        )
        .unwrap();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM payments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 2);
    }

    fn category_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM service_categories WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn seeded_services_fill_category_directory() {
        let conn = fresh();
        let categories = list_service_categories(&conn).unwrap();
        let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Фотоуслуги", "Фотопечать", "Обработка", "Полиграфия"]);

        let photo = categories.iter().find(|c| c.name == "Фотоуслуги").unwrap();
        assert_eq!(photo.service_count, 3);
        let poly = categories.iter().find(|c| c.name == "Полиграфия").unwrap();
        assert_eq!(poly.service_count, 3);
    }

    #[test]
    fn renaming_category_updates_services() {
        let conn = fresh();
        let id = category_id(&conn, "Фотоуслуги");
        save_service_category(
            &conn,
            &ServiceCategoryInput {
                id: Some(id),
                name: "Фото".into(),
            },
        )
        .unwrap();

        let services = list_services(&conn).unwrap();
        let renamed = services.iter().filter(|s| s.category == "Фото").count();
        assert_eq!(renamed, 3);
        assert!(services.iter().all(|s| s.category != "Фотоуслуги"));

        let categories = list_service_categories(&conn).unwrap();
        assert_eq!(categories.len(), 4);
        let photo = categories.iter().find(|c| c.id == id).unwrap();
        assert_eq!(photo.name, "Фото");
        assert_eq!(photo.service_count, 3);
    }

    #[test]
    fn deleting_category_keeps_services_uncategorized() {
        let conn = fresh();
        let before = list_services(&conn).unwrap().len();
        let id = category_id(&conn, "Полиграфия");
        delete_service_category(&conn, id).unwrap();

        let services = list_services(&conn).unwrap();
        assert_eq!(services.len(), before);
        assert_eq!(
            services.iter().filter(|s| s.category.is_empty()).count(),
            3
        );
        let categories = list_service_categories(&conn).unwrap();
        assert_eq!(categories.len(), 3);
        assert!(categories.iter().all(|c| c.name != "Полиграфия"));
    }

    #[test]
    fn category_names_are_case_insensitive() {
        let conn = fresh();
        let original = category_id(&conn, "Фотоуслуги");
        let ensured = ensure_service_category(&conn, "  фотоуслуги ").unwrap();
        assert_eq!(ensured, Some((original, "Фотоуслуги".to_string())));
        assert_eq!(list_service_categories(&conn).unwrap().len(), 4);

        let taken = category_name_taken(&conn, "ФОТОУСЛУГИ", None).unwrap();
        assert!(taken);
        let free = category_name_taken(&conn, "Новая", None).unwrap();
        assert!(!free);
        let own = category_name_taken(&conn, "фотоуслуги", Some(original)).unwrap();
        assert!(!own);
    }

    #[test]
    fn legacy_service_categories_are_backfilled() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy(&conn);
        conn.execute(
            "INSERT INTO services (name, unit, price_kop, category, sort_order) VALUES ('Старая услуга', 'усл.', 10000, 'Старые', 0)",
            [],
        )
        .unwrap();

        conn.execute_batch(SCHEMA).unwrap();
        migrate(&conn).unwrap();
        seed(&conn).unwrap();

        let categories = list_service_categories(&conn).unwrap();
        assert!(categories.iter().any(|c| c.name == "Старые"));
        // Повторная миграция не плодит дубликаты.
        migrate(&conn).unwrap();
        assert_eq!(
            list_service_categories(&conn)
                .unwrap()
                .iter()
                .filter(|c| c.name == "Старые")
                .count(),
            1
        );
    }
}
