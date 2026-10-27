import { api } from "../api.js";
import {
  el,
  icon,
  field,
  openModal,
  toast,
  toastError,
  debounce,
} from "../ui.js";
import {
  money,
  rub,
  parseMoney,
  parseQty,
  qtyText,
  todayIso,
  itemsWord,
  dateRu,
  shortName,
  plural,
} from "../format.js";
import { renderPdf, clearPdf } from "../pdf.js";
import { state } from "../shell.js";
import { openSellerForm, openSellersModal, switchToSeller } from "./sellers.js";

let active = null;
let uidCounter = 0;
const uid = () => `i${++uidCounter}`;

const IMAGE_EXT = /\.(png|jpe?g|webp|bmp)$/i;

const LEGAL_TITLE = "Важная правовая информация для покупателя:";
const LEGAL_BODY =
  "Настоящий документ подготовлен исключительно для удобства клиента, носит информационный характер и не является первичным учётным или расчётным документом.";
const RECEIPT_LEGAL_NOTE =
  "Данный чек является официальным подтверждением оплаты и предоставления услуги и сформирован в соответствии со ст. 14 Федерального закона № 422-ФЗ в приложении «Мой налог».";
const PENDING_TEXT =
  "Чек НПД не приложен по техническим причинам (отсутствие интернет-соединения, отсутствие связи с порталом «Мой налог» и прочее). Обязуюсь направить чек покупателю по электронной почте или в приложении «MAX» сразу после решения технических проблем.";

export function isReceiptFile(path) {
  return IMAGE_EXT.test(path ?? "");
}

export async function handleDroppedPath(path) {
  if (!path) return;
  if (!isReceiptFile(path)) {
    toast("Перетащите PNG-чек из «Мой налог»", { type: "error", title: "Неподходящий файл" });
    return;
  }
  if (active) {
    await active.dropPath(path);
  } else {
    await openWizard({
      initialPath: path,
      onSaved: () => window.dispatchEvent(new CustomEvent("cz:data-changed")),
    });
  }
}

export async function openWizard({ initialPath = null, document = null, repeat = null, onSaved = null } = {}) {
  if (active) {
    if (initialPath) await active.dropPath(initialPath);
    return active;
  }

  const settings = state.settings ?? (await api.settings());
  state.seller = state.seller ?? (await api.activeSeller());
  const editing = Boolean(document?.id);
  const items = [];

  const prefillItems = document?.items ?? repeat?.items ?? [];
  for (const it of prefillItems) {
    items.push({
      key: uid(),
      name: it.name ?? "",
      qty: qtyText(it.qtyMilli ?? 1000),
      unit: it.unit ?? settings.defaultUnit ?? "усл.",
      price: money(it.priceKop ?? 0),
    });
  }
  if (items.length === 0) {
    items.push({ key: uid(), name: "", qty: "1", unit: settings.defaultUnit || "усл.", price: "" });
  }

  let receipt = null;
  let receiptError = "";
  let receiptPending = Boolean(document?.receiptPending);
  let lastPdf = null;
  let previewSeq = 0;
  let lastPreviewWidth = 0;
  let clients = [];
  let services = [];
  let closed = false;

  /* ---------- поля ---------- */

  const numberInput = el("input", {
    class: "input",
    value: document?.number ?? "",
    placeholder: "например 23cgmf8snp",
    spellcheck: "false",
    style: { fontFamily: "ui-monospace, Consolas, monospace" },
  });
  const dateInput = el("input", { class: "input", type: "date", value: document?.docDate ?? todayIso() });
  const buyerNameInput = el("input", {
    class: "input",
    value: document?.buyerName ?? repeat?.buyerName ?? "",
    placeholder: "ФИО или название организации",
  });
  const buyerInnInput = el("input", {
    class: "input",
    value: document?.buyerInn ?? repeat?.buyerInn ?? "",
    placeholder: "Необязательно",
    inputmode: "numeric",
  });
  const noteInput = el("textarea", {
    class: "textarea",
    rows: "2",
    placeholder: "Основание, комментарий для покупателя",
  });
  noteInput.value = document?.note ?? repeat?.note ?? "";

  const buyerSelect = el(
    "select",
    { class: "select" },
    el("option", { value: "" }, "— заполнить вручную —"),
  );
  buyerSelect.addEventListener("change", () => {
    const id = buyerSelect.value ? Number(buyerSelect.value) : null;
    const client = clients.find((c) => c.id === id);
    if (client) {
      buyerNameInput.value = client.name;
      buyerInnInput.value = client.inn || "";
    }
  });

  /* ---------- чек НПД ---------- */

  const receiptZone = el("div");
  let receiptBusy = false;

  const pendingCheck = el("input", { type: "checkbox" });
  pendingCheck.checked = receiptPending;
  pendingCheck.addEventListener("change", () => {
    receiptPending = pendingCheck.checked;
    schedulePreview();
  });

  const pendingBox = el(
    "label",
    { class: "pending-receipt" },
    pendingCheck,
    el(
      "span",
      { class: "pending-receipt__text" },
      el(
        "span",
        { class: "pending-receipt__title" },
        "Чек не добавлен по техническим причинам",
      ),
      el("span", { class: "pending-receipt__hint" }, PENDING_TEXT),
    ),
  );

  function syncPendingState() {
    if (receipt) {
      receiptPending = false;
      pendingCheck.checked = false;
    }
    pendingCheck.disabled = Boolean(receipt);
    pendingBox.style.display = receipt ? "none" : "";
  }

  function renderReceipt() {
    receiptZone.innerHTML = "";
    syncPendingState();
    if (receiptBusy) {
      receiptZone.appendChild(
        el(
          "div",
          { class: "dropzone", style: { pointerEvents: "none" } },
          el("span", { html: icon("refresh", "spin"), style: { width: "20px", height: "20px" } }),
          el("div", { class: "dropzone__title" }, "Читаем QR-код…"),
        ),
      );
      return;
    }
    if (!receipt) {
      const zone = el(
        "div",
        { class: "dropzone" },
        el("span", { html: icon("upload"), style: { width: "22px", height: "22px", color: "var(--accent)" } }),
        el("div", { class: "dropzone__title" }, "Прикрепите чек из «Мой налог»"),
        el("div", { class: "dropzone__text" }, "PNG или JPG. Номер и QR будут прочитаны автоматически. Если чека нет — отметьте причину ниже."),
      );
      zone.addEventListener("click", () => pickFile());
      receiptZone.appendChild(zone);
      if (receiptError) receiptZone.appendChild(el("div", { class: "notice notice--danger", style: { marginTop: "8px" } }, el("div", { html: icon("alert") }), el("div", {}, receiptError)));
      return;
    }

    const card = el(
      "div",
      { class: "receipt-card" },
      el("img", { class: "receipt-card__img", src: receipt.previewDataUrl, alt: "Чек НПД" }),
      el(
        "div",
        { class: "receipt-card__main" },
        el(
          "div",
          { class: "row", style: { gap: "6px" } },
          el("span", { class: "receipt-card__number" }, receipt.number || "номер не распознан"),
          receipt.number
            ? el("span", { class: "badge badge--success" }, "QR прочитан")
            : el("span", { class: "badge badge--warning" }, "нет QR"),
        ),
        el(
          "div",
          { class: "receipt-card__meta" },
          receipt.inn ? el("span", {}, `ИНН ${receipt.inn}`) : null,
          receipt.width ? el("span", {}, `${receipt.width}×${receipt.height}`) : null,
          receipt.fileName ? el("span", {}, shortName(receipt.fileName, 34)) : null,
        ),
        el(
          "div",
          { class: "row", style: { gap: "6px", marginTop: "2px" } },
          el(
            "button",
            { class: "btn btn--secondary btn--sm", onclick: () => pickFile() },
            el("span", { html: icon("refresh") }),
            "Заменить",
          ),
          el(
            "button",
            {
              class: "btn btn--ghost btn--sm",
              onclick: () => {
                receipt = null;
                receiptError = "";
                renderReceipt();
                schedulePreview();
              },
            },
            "Убрать",
          ),
        ),
      ),
    );
    receiptZone.appendChild(card);
    receiptZone.appendChild(el("div", { class: "legal-note" }, RECEIPT_LEGAL_NOTE));

    if (receipt.linkedDocumentId && receipt.linkedDocumentId !== document?.id) {
      receiptZone.appendChild(
        el(
          "div",
          { class: "notice notice--warning", style: { marginTop: "8px" } },
          el("div", { html: icon("alert") }),
          el(
            "div",
            {},
            "Этот чек уже привязан к другой детализации заказа. Проверьте, не создаёте ли вы дубль.",
          ),
        ),
      );
    }
    const seller = state.seller;
    if (receipt.inn && seller) {
      if (seller.inn && receipt.inn !== seller.inn) {
        const known = (state.sellers || []).find((s) => s.inn === receipt.inn && !s.archived);
        const activeLabel = `«${seller.name || "без имени"}» (ИНН ${seller.inn})`;
        receiptZone.appendChild(
          el(
            "div",
            { class: "notice notice--warning", style: { marginTop: "8px" } },
            el("div", { html: icon("alert") }),
            el(
              "div",
              {},
              `Чек выдан самозанятому с ИНН ${receipt.inn}, а активен профиль ${activeLabel}. Пока ИНН не совпадают, документ не сохранится.`,
              el(
                "div",
                { class: "row", style: { gap: "6px", marginTop: "6px", flexWrap: "wrap" } },
                known
                  ? el(
                      "button",
                      {
                        class: "btn btn--secondary btn--sm",
                        onclick: async () => {
                          try {
                            await useSeller(known.id);
                          } catch (e) {
                            toastError(e);
                          }
                        },
                      },
                      `Переключиться на «${known.name || "без имени"}»`,
                    )
                  : null,
                el(
                  "button",
                  {
                    class: "btn btn--secondary btn--sm",
                    onclick: () =>
                      openSellerForm({
                        prefillInn: receipt.inn,
                        onSaved: (saved) => applySavedSeller(saved),
                      }),
                  },
                  known ? "Добавить новый профиль" : "Создать профиль с этим ИНН",
                ),
              ),
            ),
          ),
        );
      } else if (!seller.inn) {
        receiptZone.appendChild(
          el(
            "div",
            { class: "notice notice--warning", style: { marginTop: "8px" } },
            el("div", { html: icon("alert") }),
            el(
              "div",
              {},
              `У активного профиля не заполнен ИНН. В чеке указан ИНН ${receipt.inn} — без него документ не сохранится.`,
              el(
                "div",
                { class: "row", style: { gap: "6px", marginTop: "6px", flexWrap: "wrap" } },
                el(
                  "button",
                  {
                    class: "btn btn--secondary btn--sm",
                    onclick: () =>
                      openSellerForm({
                        seller,
                        prefillInn: receipt.inn,
                        onSaved: (saved) => applySavedSeller(saved),
                      }),
                  },
                  "Заполнить профиль",
                ),
              ),
            ),
          ),
        );
      }
    }
    if (!receipt.number) {
      receiptZone.appendChild(
        el(
          "div",
          { class: "notice notice--warning", style: { marginTop: "8px" } },
          el("div", { html: icon("alert") }),
          el("div", {}, "QR-код не распознан. Впишите номер чека вручную — он попадёт в документ."),
        ),
      );
    }
  }

  async function pickFile() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        title: "Выберите чек из «Мой налог»",
        multiple: false,
        filters: [{ name: "Чек НПД", extensions: ["png", "jpg", "jpeg"] }],
      });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (path) await loadReceipt(path);
    } catch (e) {
      toastError(e);
    }
  }

  async function loadReceipt(path) {
    if (!isReceiptFile(path)) {
      toast("Нужен файл PNG или JPG", { type: "error" });
      return;
    }
    receiptBusy = true;
    receiptError = "";
    renderReceipt();
    try {
      const imported = await api.importReceipt(path);
      receipt = imported;
      if (imported.number) {
        numberInput.value = imported.number;
      }
      if (!dateInput.value) dateInput.value = todayIso();
      if (!noteInput.value) noteInput.value = "";
      receiptError = "";
      toast(
        imported.number ? `Номер чека: ${imported.number}` : "Чек прикреплён, но QR не распознан",
        { type: imported.number ? "success" : "warning", title: "Чек НПД" },
      );
    } catch (e) {
      receipt = null;
      receiptError = String(e?.message ?? e);
      toastError(e);
    } finally {
      receiptBusy = false;
      renderReceipt();
      schedulePreview();
    }
  }

  /* ---------- позиции ---------- */

  const itemsBody = el("div", { class: "items" });
  const totalValue = el("div", { class: "wizard__total-value" }, "0,00 ₽");
  const totalCount = el("div", { class: "small muted" });

  function itemSum(item) {
    const price = parseMoney(item.price);
    const qty = parseQty(item.qty);
    if (price === null || qty === null) return 0;
    return Math.round((price * qty) / 1000);
  }

  function recalc() {
    let total = 0;
    for (const item of items) {
      const sum = itemSum(item);
      total += sum;
      item._sumEl && (item._sumEl.textContent = `${money(sum)} ₽`);
    }
    totalValue.textContent = `${money(total)} ₽`;
    totalCount.textContent = `${items.length} ${itemsWord(items.length)}`;
  }

  function renderItems() {
    itemsBody.innerHTML = "";
    for (const item of items) {
      const row = el("div", { class: "item-row" });

      const nameEl = el("input", {
        class: "input item-row__name",
        value: item.name,
        placeholder: "Наименование услуги",
        list: "wizard-services",
      });
      nameEl.addEventListener("input", () => {
        item.name = nameEl.value;
        schedulePreview();
      });

      const qtyEl = el("input", {
        class: "input input--num",
        value: item.qty,
        inputmode: "decimal",
        placeholder: "1",
      });
      qtyEl.addEventListener("input", () => {
        item.qty = qtyEl.value;
        recalc();
        schedulePreview();
      });

      const unitEl = el("input", {
        class: "input",
        value: item.unit,
        placeholder: "усл.",
      });
      unitEl.addEventListener("input", () => {
        item.unit = unitEl.value;
        schedulePreview();
      });

      const priceEl = el("input", {
        class: "input input--num",
        value: item.price,
        inputmode: "decimal",
        placeholder: "0,00",
      });
      priceEl.addEventListener("input", () => {
        item.price = priceEl.value;
        recalc();
        schedulePreview();
      });

      const sumEl = el("div", { class: "item-row__sum" }, `${money(itemSum(item))} ₽`);
      item._sumEl = sumEl;

      const removeEl = el(
        "button",
        {
          class: "btn btn--ghost btn--icon item-row__remove",
          title: "Убрать позицию",
          onclick: () => {
            const idx = items.indexOf(item);
            if (idx >= 0) items.splice(idx, 1);
            if (items.length === 0) {
              addItem();
            } else {
              renderItems();
              schedulePreview();
            }
          },
        },
        el("span", { html: icon("x") }),
      );

      const meta = el(
        "div",
        { class: "item-row__meta" },
        el(
          "label",
          { class: "item-row__cell" },
          el("span", { class: "item-row__label" }, "Кол-во"),
          qtyEl,
        ),
        el("label", { class: "item-row__cell" }, el("span", { class: "item-row__label" }, "Ед."), unitEl),
        el("label", { class: "item-row__cell" }, el("span", { class: "item-row__label" }, "Цена"), priceEl),
        el(
          "div",
          { class: "item-row__cell" },
          el("span", { class: "item-row__label" }, "Сумма"),
          sumEl,
        ),
      );

      row.append(nameEl, removeEl, meta);
      itemsBody.appendChild(row);
    }
    recalc();
  }

  function addItem(data = {}) {
    items.push({
      key: uid(),
      name: data.name ?? "",
      qty: data.qty ?? "1",
      unit: data.unit ?? settings.defaultUnit ?? "усл.",
      price: data.price ?? "",
    });
    renderItems();
    schedulePreview();
    if (items.length > 1) {
      itemsBody.lastElementChild?.querySelector("input")?.focus();
    }
  }

  const servicesDatalist = el("datalist", { id: "wizard-services" });

  const catalog = el("div", { class: "catalog" });

  function renderCatalog() {
    catalog.innerHTML = "";
    if (services.length === 0) {
      catalog.appendChild(el("span", { class: "small muted" }, "Каталог услуг пуст — добавьте шаблоны в разделе «Услуги»."));
      return;
    }
    for (const service of services.slice(0, 14)) {
      const chip = el(
        "button",
        {
          type: "button",
          class: "catalog__chip",
          title: service.category || "",
          onclick: () => {
            const last = items[items.length - 1];
            const isEmpty =
              last &&
              !last.name.trim() &&
              !parseMoney(last.price) &&
              (parseQty(last.qty) ?? 0) === 1000;
            if (isEmpty) {
              last.name = service.name;
              last.unit = service.unit;
              last.price = money(service.priceKop);
              itemsBody.innerHTML = "";
              renderItems();
              schedulePreview();
            } else {
              addItem({ name: service.name, unit: service.unit, price: money(service.priceKop) });
            }
          },
        },
        service.name,
        el("span", { class: "catalog__price" }, `${money(service.priceKop)} ₽`),
      );
      catalog.appendChild(chip);
    }
  }

  /* ---------- предпросмотр ---------- */

  const previewBody = el("div", { class: "wizard__preview-body" });
  const previewStatus = el("div", { class: "small muted" });
  const previewPages = el("span", { class: "badge" }, "A4");

  function placeholder(text, iconName = "file") {
    clearPdf(previewBody);
    previewBody.innerHTML = "";
    previewBody.appendChild(
      el(
        "div",
        { class: "wizard__canvas--placeholder" },
        el(
          "div",
          { style: { display: "flex", flexDirection: "column", alignItems: "center", gap: "8px" } },
          el("span", { html: icon(iconName), style: { width: "26px", height: "26px" } }),
          el("span", {}, text),
        ),
      ),
    );
  }

  function buildInput() {
    const cleaned = items
      .map((it) => ({
        name: it.name.trim(),
        qtyMilli: parseQty(it.qty) ?? 0,
        unit: it.unit.trim(),
        priceKop: parseMoney(it.price) ?? 0,
      }))
      .filter((it) => it.name || it.priceKop);
    return {
      id: document?.id ?? null,
      number: numberInput.value.trim(),
      docDate: dateInput.value,
      buyerName: buyerNameInput.value.trim(),
      buyerInn: buyerInnInput.value.trim(),
      note: noteInput.value.trim(),
      receiptFileId: receipt?.fileId ?? null,
      receiptPending: receipt ? false : receiptPending,
      qrPayload: receipt?.payload ?? "",
      items: cleaned,
    };
  }

  function canPreview(input) {
    const hasReceipt = Boolean(input.receiptFileId);
    return (
      (hasReceipt || input.receiptPending) &&
      (!hasReceipt || input.number.length > 0) &&
      input.docDate &&
      input.items.length > 0 &&
      input.items.every((it) => it.name.length > 0 && it.priceKop > 0 && it.qtyMilli > 0)
    );
  }

  async function refreshPreview() {
    if (closed) return;
    const input = buildInput();
    if (!canPreview(input)) {
      lastPdf = null;
      placeholder(
        receipt
          ? "Заполните номер и хотя бы одну позицию"
          : receiptPending
            ? "Заполните дату и хотя бы одну позицию"
            : "Прикрепите чек НПД или отметьте, что пришлёте его позже",
        receipt || receiptPending ? "edit" : "qr",
      );
      previewStatus.textContent = "Предпросмотр появится автоматически";
      previewPages.textContent = "A4";
      return;
    }
    const seq = ++previewSeq;
    previewStatus.textContent = "Формируем PDF…";
    try {
      const base64 = await api.previewDocument(input);
      if (closed || seq !== previewSeq) return;
      lastPdf = base64;
      lastPreviewWidth = previewBody.clientWidth;
      const pages = await renderPdf(previewBody, base64, { padding: 2 });
      if (closed || seq !== previewSeq) return;
      previewPages.textContent = `${pages} ${plural(pages, "страница", "страницы", "страниц")} · A4`;
      previewStatus.textContent = receipt
        ? "PDF с оригинальным чеком НПД"
        : "PDF без чека НПД, с примечанием для покупателя";
    } catch (e) {
      if (closed || seq !== previewSeq) return;
      lastPdf = null;
      previewBody.innerHTML = "";
      previewBody.appendChild(
        el(
          "div",
          { class: "notice notice--danger", style: { margin: "18px", maxWidth: "360px" } },
          el("div", { html: icon("alert") }),
          el("div", {}, String(e?.message ?? e)),
        ),
      );
      previewStatus.textContent = "Исправьте данные";
    }
  }

  const schedulePreview = debounce(refreshPreview, 450);

  /* ---------- активный самозанятый ---------- */

  const sellerBar = el("div", { class: "wizard__seller" });

  const pdfHint = el("div", { class: "small muted", style: { marginLeft: "2px" } });

  function renderPdfHint() {
    const folder = state.seller?.folder;
    pdfHint.textContent = folder
      ? `PDF сохраняется в «Документы\\Чеки\\${folder}»`
      : "PDF сохраняется в папку «Документы\\Чеки»";
  }

  function renderSellerBar() {
    const seller = state.seller;
    sellerBar.innerHTML = "";
    renderPdfHint();
    sellerBar.append(
      el("span", { class: "wizard__seller-icon", html: icon("users") }),
      el(
        "span",
        { class: "wizard__seller-text" },
        el(
          "span",
          { class: "wizard__seller-name" },
          el("span", { class: "muted" }, "Продавец: "),
          seller?.name || "профиль не выбран",
        ),
        seller?.inn
          ? el("span", { class: "wizard__seller-meta" }, `ИНН ${seller.inn}`)
          : el("span", { class: "badge badge--warning" }, "ИНН не заполнен"),
      ),
      el(
        "button",
        { class: "btn btn--ghost btn--sm", type: "button", onclick: () => openSellersModal() },
        "Сменить",
      ),
    );
  }

  renderSellerBar();

  async function useSeller(id) {
    const seller = await switchToSeller(id);
    renderSellerBar();
    if (editing) {
      closed = true;
      modal.close();
      toast(
        `Профиль: ${seller.name || "без имени"}. Документ принадлежит другому самозанятому — откройте его заново.`,
        { type: "info", timeout: 6000 },
      );
      return;
    }
    renderReceipt();
    schedulePreview();
  }

  async function applySavedSeller(saved) {
    try {
      if (saved && saved.inn && saved.inn === receipt?.inn && state.seller?.id !== saved.id) {
        await useSeller(saved.id);
        return;
      }
      renderSellerBar();
      renderReceipt();
      schedulePreview();
    } catch (e) {
      toastError(e);
    }
  }

  const onSellerChanged = () => {
    if (closed) return;
    renderSellerBar();
    renderReceipt();
    schedulePreview();
  };
  window.addEventListener("cz:seller-changed", onSellerChanged);

  /* ---------- сборка ---------- */

  const receiptSection = el(
    "section",
    { class: "section" },
    el(
      "div",
      { class: "section__head" },
      el(
        "div",
        { class: "section__title" },
        "Чек НПД ",
        el("span", { class: "badge badge--outline" }, "необязательно"),
      ),
      el(
        "div",
        { class: "section__actions" },
        el(
          "button",
          { class: "btn btn--ghost btn--sm", onclick: () => pickFile() },
          el("span", { html: icon("upload") }),
          "Из файла",
        ),
      ),
    ),
    sellerBar,
    receiptZone,
    pendingBox,
  );

  const legalNotice = el(
    "div",
    { class: "legal-note legal-note--warn" },
    el("span", { class: "legal-note__mark" }, "⚠️"),
    el(
      "div",
      {},
      el("div", { class: "legal-note__title" }, LEGAL_TITLE),
      el("div", {}, LEGAL_BODY),
    ),
  );

  const detailsSection = el(
    "section",
    { class: "section section--bordered" },
    el("div", { class: "section__head" }, el("div", { class: "section__title" }, "Реквизиты документа")),
    el(
      "div",
      { class: "settings-row" },
      field(
        "Номер чека НПД (если известен)",
        numberInput,
        "Подставляется из QR. Без чека можно не заполнять",
      ),
      field("Дата", dateInput, "По умолчанию — сегодня"),
    ),
    el(
      "div",
      { class: "settings-row" },
      field("Клиент из справочника", buyerSelect),
      field("Покупатель", buyerNameInput),
    ),
    field("ИНН покупателя", buyerInnInput, "Необязательно"),
    field("Комментарий", noteInput, "Попадёт в документ, если заполнено"),
  );

  const itemsSection = el(
    "section",
    { class: "section section--bordered" },
    el(
      "div",
      { class: "section__head" },
      el("div", { class: "section__title" }, "Позиции"),
      el(
        "div",
        { class: "section__actions" },
        el(
          "button",
          { class: "btn btn--secondary btn--sm", onclick: () => addItem() },
          el("span", { html: icon("plus") }),
          "Позиция",
        ),
      ),
    ),
    catalog,
    itemsBody,
    servicesDatalist,
  );

  const totalBar = el(
    "div",
    { class: "wizard__total" },
    el(
      "div",
      {},
      el("div", { class: "wizard__total-label" }, "Итого по документу"),
      totalCount,
    ),
    totalValue,
  );

  const formCol = el(
    "div",
    { class: "wizard__form" },
    legalNotice,
    receiptSection,
    detailsSection,
    itemsSection,
    totalBar,
  );

  const previewHead = el(
    "div",
    { class: "wizard__preview-head" },
    el("span", { html: icon("file"), style: { width: "15px", height: "15px", color: "var(--muted)" } }),
    el("div", { class: "small", style: { fontWeight: "600" } }, "Предпросмотр документа"),
    previewPages,
    el("div", { style: { marginLeft: "auto", display: "flex", alignItems: "center", gap: "8px" } }, previewStatus,
      el(
        "button",
        {
          class: "btn btn--ghost btn--icon btn--sm",
          title: "Обновить предпросмотр",
          onclick: () => refreshPreview(),
        },
        el("span", { html: icon("refresh") }),
      ),
    ),
  );

  const previewCol = el("div", { class: "wizard__preview" }, previewHead, previewBody);
  const wizard = el("div", { class: "wizard" }, formCol, previewCol);

  const errorEl = el("div", { class: "modal__error" });

  const modal = openModal({
    title: editing ? "Детализация заказа" : "Новая детализация заказа",
    sub: "Официальное подтверждение оплаты — чек НПД. Его можно прикрепить сейчас или отправить покупателю позже",
    size: "full",
    body: wizard,
    onClose: () => {
      closed = true;
      window.removeEventListener("resize", onResize);
      window.removeEventListener("cz:seller-changed", onSellerChanged);
      active = null;
    },
    footer: [
      errorEl,
      pdfHint,
      el("div", { class: "spacer" }),
      el("button", { class: "btn btn--secondary", onclick: () => modal.close() }, "Отмена"),
      el(
        "button",
        {
          class: "btn btn--primary",
          onclick: async (e) => {
            errorEl.textContent = "";
            const input = buildInput();
            if (!receipt?.fileId && !input.receiptPending) {
              errorEl.textContent =
                "Прикрепите чек НПД или отметьте, что чек не добавлен по техническим причинам";
              receiptZone.scrollIntoView({ behavior: "smooth", block: "center" });
              return;
            }
            if (receipt?.fileId && !input.number) {
              errorEl.textContent = "Укажите номер чека НПД";
              numberInput.focus();
              return;
            }
            if (!input.docDate) {
              errorEl.textContent = "Укажите дату";
              dateInput.focus();
              return;
            }
            if (!state.seller?.inn) {
              errorEl.textContent =
                "Заполните ИНН самозанятого в профиле — он нужен для PDF и проверки чека";
              return;
            }
            const cleaned = input.items.filter((it) => it.name || it.priceKop);
            if (cleaned.length === 0) {
              errorEl.textContent = "Добавьте хотя бы одну позицию";
              return;
            }
            if (cleaned.some((it) => !it.name || it.priceKop <= 0 || it.qtyMilli <= 0)) {
              errorEl.textContent = "Проверьте наименование, количество и цену позиций";
              return;
            }
            const btn = e.currentTarget;
            btn.disabled = true;
            try {
              const result = await api.saveDocument(input);
              closed = true;
              modal.close();
              toast(`Детализация заказа${input.number ? ` № ${input.number}` : ""} сохранена`, {
                type: "success",
                title: rub(cleaned.reduce((s, it) => s + Math.round((it.priceKop * it.qtyMilli) / 1000), 0)),
                timeout: 9000,
                actions: [
                  { label: "Открыть PDF", onClick: () => api.openPath(result.pdfPath) },
                  { label: "Показать в папке", onClick: () => api.revealPath(result.pdfPath) },
                ],
              });
              api.openPath(result.pdfPath).catch((err) => toastError(err));
              onSaved?.();
            } catch (err) {
              errorEl.textContent = String(err?.message ?? err);
              btn.disabled = false;
            }
          },
        },
        el("span", { html: icon("printer") }),
        "Сохранить и открыть",
      ),
    ],
  });

  modal.bodyEl.classList.add("modal__body--flush");

  active = {
    modal,
    dropPath: async (path) => {
      await loadReceipt(path);
      modal.root.animate(
        [{ boxShadow: "0 0 0 3px var(--accent-soft)" }, { boxShadow: "var(--shadow-3)" }],
        { duration: 500 },
      );
    },
  };

  renderReceipt();
  renderItems();
  renderCatalog();
  placeholder("Заполните данные для предпросмотра", "edit");

  /* ---------- данные справочников ---------- */

  try {
    [clients, services] = await Promise.all([api.clients(false), api.services()]);
  } catch (e) {
    toastError(e);
  }
  if (closed) return active;

  for (const client of clients) {
    buyerSelect.appendChild(
      el("option", { value: String(client.id) }, `${client.name}${client.inn ? ` · ${client.inn}` : ""}`),
    );
  }
  const currentBuyer = document?.buyerName ?? repeat?.buyerName ?? "";
  const matched = clients.find((c) => c.name === currentBuyer);
  if (matched) buyerSelect.value = String(matched.id);

  servicesDatalist.innerHTML = "";
  for (const service of services) {
    servicesDatalist.appendChild(el("option", { value: service.name }));
  }
  renderCatalog();

  /* ---------- начальный чек ---------- */

  if (initialPath) {
    await loadReceipt(initialPath);
  } else if (editing && document?.receiptFileId) {
    try {
      const preview = await api.receiptDataUrl(document.receiptFileId);
      receipt = {
        fileId: document.receiptFileId,
        previewDataUrl: preview,
        number: document.number,
        inn: document.sellerInn,
        payload: document.qrPayload || "",
        width: 0,
        height: 0,
        fileName: "",
        duplicateFileId: document.receiptFileId,
        linkedDocumentId: document.id,
      };
      renderReceipt();
    } catch (e) {
      toastError(e);
    }
  }

  function onResize() {
    if (!lastPdf || closed) return;
    const width = previewBody.clientWidth;
    if (Math.abs(width - lastPreviewWidth) < 40) return;
    lastPreviewWidth = width;
    renderPdf(previewBody, lastPdf, { padding: 2 });
  }
  window.addEventListener("resize", onResize);

  schedulePreview();
  setTimeout(() => numberInput.focus(), 60);

  return active;
}
