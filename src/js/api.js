import { invoke } from "@tauri-apps/api/core";

const call = (cmd, args) => invoke(cmd, args);

export const api = {
  settings: () => call("get_settings"),
  saveSettings: (settings) => call("save_settings", { settings }),
  appPaths: () => call("app_paths"),

  sellers: (includeArchived = false) => call("list_sellers", { includeArchived }),
  activeSeller: () => call("active_seller"),
  saveSeller: (input) => call("save_seller", { input }),
  switchSeller: (id) => call("switch_seller", { id }),
  setSellerArchived: (id, archived) => call("set_seller_archived", { id, archived }),
  deleteSeller: (id) => call("delete_seller", { id }),

  dashboard: () => call("get_dashboard"),
  months: () => call("list_months"),
  setPaid: (year, month, paid) => call("set_paid", { year, month, paid }),

  entries: (filter) => call("list_entries", { filter }),
  saveEntry: (input) => call("save_entry", { input }),
  deleteEntry: (id) => call("delete_entry", { id }),

  clients: (includeArchived = false) => call("list_clients", { includeArchived }),
  saveClient: (input) => call("save_client", { input }),
  setClientArchived: (id, archived) => call("set_client_archived", { id, archived }),
  deleteClient: (id) => call("delete_client", { id }),

  services: () => call("list_services"),
  saveService: (input) => call("save_service", { input }),
  deleteService: (id) => call("delete_service", { id }),

  serviceCategories: () => call("list_service_categories"),
  saveServiceCategory: (input) => call("save_service_category", { input }),
  deleteServiceCategory: (id) => call("delete_service_category", { id }),

  importReceipt: (path) => call("import_receipt", { path }),
  receiptDataUrl: (fileId) => call("receipt_data_url", { fileId }),
  receiptThumb: (fileId) => call("receipt_thumb_data_url", { fileId }),

  previewDocument: (input) => call("preview_document_pdf", { input }),
  saveDocument: (input) => call("save_document", { input }),
  documents: (search) => call("list_documents", { search: search || null }),
  document: (id) => call("get_document", { id }),
  documentPdf: (id) => call("document_pdf", { id }),
  deleteDocument: (id) => call("delete_document", { id }),

  exportCsv: (path, dateFrom, dateTo) =>
    call("export_csv", {
      path,
      dateFrom: dateFrom || null,
      dateTo: dateTo || null,
    }),
  backup: (targetDir) => call("backup", { targetDir }),
  openPath: (path) => call("open_path", { path }),
  revealPath: (path) => call("reveal_path", { path }),
};
