import { api } from "../api.js";
import {
  el,
  icon,
  emptyState,
  toast,
  toastError,
  confirmDialog,
  debounce,
} from "../ui.js";
import {
  money,
  rub,
  dateRu,
  monthLabel,
  entriesWord,
  payerLabel,
  isoDate,
} from "../format.js";
import { setHeader } from "../shell.js";
import { openEntryModal } from "./entryModal.js";

const PERIODS = [
  ["month", "Месяц"],
  ["quarter", "Квартал"],
  ["year", "Год"],
  ["all", "Всё время"],
];

let period = localStorage.getItem("cz_income_period") || "month";
let anchor = new Date();
let payer = "all";
let search = "";

function daysInMonth(year, month) {
  return new Date(year, month, 0).getDate();
}

function bounds() {
  const y = anchor.getFullYear();
  const m = anchor.getMonth() + 1;
  if (period === "month") {
    return {
      from: isoDate(y, m, 1),
      to: isoDate(y, m, daysInMonth(y, m)),
      label: monthLabel(y, m),
      key: `${y}-${String(m).padStart(2, "0")}`,
    };
  }
  if (period === "quarter") {
    const qFrom = Math.floor((m - 1) / 3) * 3 + 1;
    const qTo = qFrom + 2;
    return {
      from: isoDate(y, qFrom, 1),
      to: isoDate(y, qTo, daysInMonth(y, qTo)),
      label: `${["I", "II", "III", "IV"][Math.floor((m - 1) / 3)]} квартал ${y}`,
      key: `${y}-Q${Math.floor((m - 1) / 3) + 1}`,
    };
  }
  if (period === "year") {
    return {
      from: isoDate(y, 1, 1),
      to: isoDate(y, 12, 31),
      label: `${y} год`,
      key: String(y),
    };
  }
  return { from: null, to: null, label: "Всё время", key: "all" };
}

function shift(direction) {
  if (period === "all") return;
  if (period === "month") {
    anchor = new Date(anchor.getFullYear(), anchor.getMonth() + direction, 1);
  } else if (period === "quarter") {
    anchor = new Date(anchor.getFullYear(), anchor.getMonth() + direction * 3, 1);
  } else {
    anchor = new Date(anchor.getFullYear() + direction, anchor.getMonth(), 1);
  }
}

function monthKey(year, month) {
  return `${year}-${String(month).padStart(2, "0")}`;
}

function taxForPeriod(months, from, to) {
  return months
    .filter((m) => {
      if (!from || !to) return true;
      const key = monthKey(m.year, m.month);
      return key >= from.slice(0, 7) && key <= to.slice(0, 7);
    })
    .reduce((sum, m) => sum + m.taxKop, 0);
}

export async function render(root) {
  setHeader({
    title: "Доходы",
    sub: "Налог на профессиональный доход считается автоматически",
    actions: el(
      "button",
      {
        class: "btn btn--primary",
        onclick: () =>
          openEntryModal({
            onSaved: () => {
              refresh();
            },
          }),
      },
      el("span", { html: icon("plus") }),
      "Новый доход",
    ),
  });

  root.innerHTML = "";

  const page = el("div", { class: "stack", style: { gap: "14px" } });
  const periodBar = el("div", { class: "segmented" });
  const periodButtons = [];
  for (const [value, label] of PERIODS) {
    const btn = el(
      "button",
      { type: "button", class: value === period ? "is-active" : "" },
      label,
    );
    btn.addEventListener("click", () => {
      period = value;
      localStorage.setItem("cz_income_period", period);
      periodButtons.forEach((b) => b.classList.remove("is-active"));
      btn.classList.add("is-active");
      anchor = new Date();
      syncLabel();
      refresh();
    });
    periodButtons.push(btn);
    periodBar.appendChild(btn);
  }

  const labelEl = el("div", { style: { fontWeight: "600", fontSize: "13px", minWidth: "150px" } });
  const prevBtn = el(
    "button",
    { class: "btn btn--secondary btn--icon btn--sm", title: "Предыдущий период" },
    el("span", { html: icon("left") }),
  );
  const nextBtn = el(
    "button",
    { class: "btn btn--secondary btn--icon btn--sm", title: "Следующий период" },
    el("span", { html: icon("right") }),
  );
  const nowBtn = el("button", { class: "btn btn--ghost btn--sm" }, "Текущий");
  prevBtn.addEventListener("click", () => {
    shift(-1);
    syncLabel();
    refresh();
  });
  nextBtn.addEventListener("click", () => {
    shift(1);
    syncLabel();
    refresh();
  });
  nowBtn.addEventListener("click", () => {
    anchor = new Date();
    syncLabel();
    refresh();
  });

  const searchInput = el("input", {
    class: "input",
    type: "search",
    placeholder: "Поиск по описанию, чеку, клиенту",
    value: search,
  });
  searchInput.addEventListener(
    "input",
    debounce(() => {
      search = searchInput.value.trim();
      refresh();
    }, 260),
  );

  const payerSelect = el(
    "select",
    { class: "select", style: { width: "150px" } },
    el("option", { value: "all" }, "Все плательщики"),
    el("option", { value: "individual" }, "Физлица"),
    el("option", { value: "organization" }, "Юрлица"),
  );
  payerSelect.value = payer;
  payerSelect.addEventListener("change", () => {
    payer = payerSelect.value;
    refresh();
  });

  const exportBtn = el(
    "button",
    { class: "btn btn--secondary" },
    el("span", { html: icon("download") }),
    "CSV",
  );
  exportBtn.addEventListener("click", () => exportCsv());

  const toolbar = el(
    "div",
    { class: "toolbar", style: { marginBottom: "0" } },
    periodBar,
    el("div", { class: "row", style: { gap: "4px" } }, prevBtn, labelEl, nextBtn, nowBtn),
    el("div", { class: "toolbar__spacer" }),
    el(
      "div",
      { class: "toolbar__search" },
      el("span", { html: icon("search") }),
      searchInput,
    ),
    payerSelect,
    exportBtn,
  );

  const summary = el("div", { class: "card" });
  const tableCard = el("div", { class: "card" });
  page.append(toolbar, summary, tableCard);
  root.appendChild(page);

  function syncLabel() {
    const b = bounds();
    labelEl.textContent = b.label;
    const disabled = period === "all";
    prevBtn.disabled = disabled;
    nextBtn.disabled = disabled;
    nowBtn.style.display = disabled ? "none" : "";
  }

  async function exportCsv() {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const b = bounds();
      const stamp = b.from ? b.from.slice(0, 7) : "all";
      const path = await save({
        title: "Экспорт доходов в CSV",
        defaultPath: `Доходы_${stamp}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return;
      await api.exportCsv(path, b.from, b.to);
      toast("Файл сохранён", {
        type: "success",
        title: path,
        actions: [{ label: "Открыть папку", onClick: () => api.revealPath(path) }],
      });
    } catch (e) {
      toastError(e);
    }
  }

  async function refresh() {
    const b = bounds();
    summary.innerHTML = "";
    tableCard.innerHTML = "";
    summary.appendChild(
      el(
        "div",
        { class: "card__body", style: { display: "flex", gap: "28px", flexWrap: "wrap" } },
        el("div", { class: "loading-line small muted" }, "Считаем…"),
      ),
    );
    syncLabel();

    let entries = [];
    let months = [];
    try {
      [entries, months] = await Promise.all([
        api.entries({
          dateFrom: b.from,
          dateTo: b.to,
          payerType: payer,
          search: search || null,
          limit: null,
        }),
        api.months(),
      ]);
    } catch (e) {
      summary.innerHTML = "";
      tableCard.innerHTML = "";
      tableCard.appendChild(
        el(
          "div",
          { class: "notice notice--danger", style: { margin: "16px" } },
          el("div", { html: icon("alert") }),
          el("div", {}, String(e?.message ?? e)),
        ),
      );
      return;
    }

    const income = entries.reduce((s, e) => s + e.amountKop, 0);
    const tax = taxForPeriod(months, b.from, b.to);
    const average = entries.length ? Math.round(income / entries.length) : 0;

    summary.innerHTML = "";
    summary.appendChild(
      el(
        "div",
        { class: "summary" },
        summaryItem("Доход за период", `${money(income)} ₽`, "wallet"),
        summaryItem("Налог за период", `${money(tax)} ₽`, "receipt"),
        summaryItem("Операций", String(entries.length), "list", `${entriesWord(entries.length)}`),
        summaryItem("Средний доход", `${money(average)} ₽`, "chart"),
      ),
    );

    const head = el(
      "div",
      { class: "card__head" },
      el("h3", { class: "card__title" }, b.label),
      el("span", { class: "card__sub" }, `${entries.length} ${entriesWord(entries.length)}`),
      el(
        "div",
        { class: "card__actions" },
        el(
          "button",
          {
            class: "btn btn--ghost btn--sm",
            onclick: () =>
              openEntryModal({
                onSaved: () => refresh(),
              }),
          },
          el("span", { html: icon("plus") }),
          "Добавить",
        ),
      ),
    );
    tableCard.appendChild(head);

    if (entries.length === 0) {
      tableCard.appendChild(
        emptyState({
          iconName: "wallet",
          title: search || payer !== "all" ? "Ничего не найдено" : "За этот период доходов нет",
          text:
            search || payer !== "all"
              ? "Попробуйте изменить фильтры или период."
              : "Добавьте первую операцию — налог посчитается автоматически.",
          action: el(
            "button",
            {
              class: "btn btn--primary btn--sm",
              onclick: () =>
                openEntryModal({
                  onSaved: () => refresh(),
                }),
            },
            "Добавить доход",
          ),
        }),
      );
      return;
    }

    const table = el(
      "table",
      { class: "table" },
      el(
        "thead",
        {},
        el(
          "tr",
          {},
          el("th", { style: { width: "96px" } }, "Дата"),
          el("th", {}, "Операция"),
          el("th", { style: { width: "110px" } }, "Плательщик"),
          el("th", { style: { width: "120px" } }, "Чек НПД"),
          el("th", { class: "col-num", style: { width: "130px" } }, "Сумма"),
          el("th", { class: "col-actions", style: { width: "76px" } }),
        ),
      ),
    );
    const tbody = el("tbody");
    for (const entry of entries) {
      tbody.appendChild(row(entry));
    }
    table.appendChild(tbody);

    const body = el("div", { class: "table-wrap", style: { maxHeight: "calc(100vh - 380px)" } }, table);
    tableCard.appendChild(body);
    tableCard.appendChild(
      el(
        "div",
        { class: "table-foot" },
        el("span", {}, "Итого: ", el("strong", {}, `${money(income)} ₽`)),
        el("span", {}, "Налог: ", el("strong", {}, `${money(tax)} ₽`)),
        el(
          "span",
          { style: { marginLeft: "auto" } },
          "Вычет применяется автоматически к доходам от юрлиц",
        ),
      ),
    );
  }

  function row(entry) {
    const tr = el(
      "tr",
      { style: { cursor: "pointer" } },
      el("td", { class: "cell-muted nowrap" }, dateRu(entry.date)),
      el(
        "td",
        {},
        el(
          "div",
          { class: "cell-title" },
          el("span", { class: "cell-strong" }, entry.description || "Доход"),
          entry.clientName
            ? el("small", {}, entry.clientName)
            : null,
        ),
      ),
      el(
        "td",
        {},
        el(
          "span",
          { class: entry.payerType === "organization" ? "badge badge--accent" : "badge" },
          payerLabel(entry.payerType),
        ),
      ),
      el(
        "td",
        {},
        entry.receiptIssued
          ? el(
              "span",
              { class: "badge badge--success", title: entry.receiptNo || "" },
              el("span", { html: icon("check"), style: { width: "11px", height: "11px" } }),
              entry.receiptNo || "выдан",
            )
          : el("span", { class: "badge badge--warning" }, "не выдан"),
      ),
      el("td", { class: "col-num cell-strong" }, `${money(entry.amountKop)} ₽`),
      el(
        "td",
        { class: "col-actions" },
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Изменить",
            onclick: (e) => {
              e.stopPropagation();
              openEntryModal({ entry, onSaved: () => refresh() });
            },
          },
          el("span", { html: icon("edit") }),
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Удалить",
            onclick: async (e) => {
              e.stopPropagation();
              const ok = await confirmDialog({
                title: "Удалить операцию?",
                text: `Доход от ${dateRu(entry.date)} на ${rub(entry.amountKop)} будет удалён. Налог пересчитается.`,
                confirmText: "Удалить",
                danger: true,
              });
              if (!ok) return;
              try {
                await api.deleteEntry(entry.id);
                toast("Операция удалена", { type: "success" });
                refresh();
              } catch (err) {
                toastError(err);
              }
            },
          },
          el("span", { html: icon("trash") }),
        ),
      ),
    );
    tr.addEventListener("click", () => openEntryModal({ entry, onSaved: () => refresh() }));
    return tr;
  }

  await refresh();
}

function summaryItem(label, value, iconName, hint = "") {
  return el(
    "div",
    { class: "summary__item" },
    el(
      "div",
      { class: "summary__label" },
      el("span", { html: icon(iconName), style: { width: "13px", height: "13px" } }),
      label,
    ),
    el("div", { class: "summary__value" }, value),
    hint ? el("div", { class: "summary__hint" }, hint) : null,
  );
}
