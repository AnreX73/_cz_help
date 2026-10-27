use serde::{Deserialize, Serialize};

pub const PAYER_INDIVIDUAL: &str = "individual";
pub const PAYER_ORGANIZATION: &str = "organization";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: i64,
    pub name: String,
    pub inn: String,
    pub kind: String,
    pub note: String,
    pub archived: bool,
    pub total_kop: i64,
    pub entries_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInput {
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub inn: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub note: String,
}

fn default_kind() -> String {
    PAYER_INDIVIDUAL.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seller {
    pub id: i64,
    pub name: String,
    pub inn: String,
    pub contact: String,
    pub city: String,
    pub status: String,
    /// Папка для PDF внутри каталога «Чеки». Задаётся один раз при создании,
    /// поэтому переименование профиля не ломает пути к уже сохранённым файлам.
    pub folder: String,
    pub archived: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerInput {
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub inn: String,
    #[serde(default)]
    pub contact: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub seller_id: i64,
    pub date: String,
    pub amount_kop: i64,
    pub payer_type: String,
    pub client_id: Option<i64>,
    pub client_name: Option<String>,
    pub description: String,
    pub receipt_no: String,
    pub receipt_issued: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub id: Option<i64>,
    pub date: String,
    pub amount_kop: i64,
    pub payer_type: String,
    pub client_id: Option<i64>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub receipt_no: String,
    #[serde(default)]
    pub receipt_issued: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: i64,
    pub name: String,
    pub unit: String,
    pub price_kop: i64,
    pub category: String,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInput {
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub price_kop: i64,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCategory {
    pub id: i64,
    pub name: String,
    pub service_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCategoryInput {
    pub id: Option<i64>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentItem {
    pub id: i64,
    pub document_id: i64,
    pub position: i64,
    pub name: String,
    pub qty_milli: i64,
    pub unit: String,
    pub price_kop: i64,
    pub sum_kop: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentItemInput {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    #[serde(default = "one_milli")]
    pub qty_milli: i64,
    #[serde(default)]
    pub unit: String,
    pub price_kop: i64,
}

fn one_milli() -> i64 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: i64,
    pub seller_id: i64,
    pub number: String,
    pub doc_date: String,
    pub buyer_name: String,
    pub buyer_inn: String,
    pub total_kop: i64,
    pub note: String,
    pub seller_name: String,
    pub seller_inn: String,
    pub receipt_file_id: Option<i64>,
    pub receipt_path: Option<String>,
    /// Чек НПД не приложен: продавец подтвердил, что пришлёт его покупателю
    /// после устранения технических проблем.
    pub receipt_pending: bool,
    pub qr_payload: String,
    pub created_at: String,
    pub items: Vec<DocumentItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInput {
    pub id: Option<i64>,
    pub number: String,
    pub doc_date: String,
    #[serde(default)]
    pub buyer_name: String,
    #[serde(default)]
    pub buyer_inn: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub receipt_file_id: Option<i64>,
    #[serde(default)]
    pub receipt_pending: bool,
    #[serde(default)]
    pub qr_payload: String,
    #[serde(default)]
    pub items: Vec<DocumentItemInput>,
    #[serde(default)]
    pub seller_name: Option<String>,
    #[serde(default)]
    pub seller_inn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub rate_individual: i64,
    pub rate_organization: i64,
    pub deduction_kop: i64,
    pub limit_kop: i64,
    pub receipt_width_mm: i64,
    pub extra_qr: bool,
    pub theme: String,
    pub default_unit: String,
    pub documents_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rate_individual: 400,
            rate_organization: 600,
            deduction_kop: 1_000_000,
            limit_kop: 240_000_000,
            receipt_width_mm: 70,
            extra_qr: true,
            theme: "light".into(),
            default_unit: "усл.".into(),
            documents_dir: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptImport {
    pub path: String,
    pub file_name: String,
    pub file_id: i64,
    pub sha256: String,
    pub width: i64,
    pub height: i64,
    pub number: Option<String>,
    pub inn: Option<String>,
    pub payload: Option<String>,
    pub preview_data_url: String,
    pub duplicate_file_id: Option<i64>,
    pub linked_document_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthReport {
    pub year: i64,
    pub month: i64,
    pub income_individual_kop: i64,
    pub income_organization_kop: i64,
    pub income_total_kop: i64,
    pub tax_kop: i64,
    pub deduction_used_kop: i64,
    pub entries_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub settings: Settings,
    pub seller: Seller,
    pub month: PeriodTotals,
    pub quarter: PeriodTotals,
    pub year: PeriodTotals,
    pub deduction_left_kop: i64,
    pub deduction_total_kop: i64,
    pub limit_left_kop: i64,
    pub limit_total_kop: i64,
    pub limit_ratio: f64,
    pub recent: Vec<Entry>,
    pub unissued_receipts: i64,
    pub documents_count: i64,
    pub due: Option<PaymentDue>,
    pub months: Vec<MonthReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PeriodTotals {
    pub label: String,
    pub income_kop: i64,
    pub tax_kop: i64,
    pub entries_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDue {
    pub year: i64,
    pub month: i64,
    pub tax_kop: i64,
    pub due_date: String,
    pub paid: bool,
}
