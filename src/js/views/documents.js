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
  itemsWord,
  shortName,
} from "../format.js";
import { setHeader } from "../shell.js";
import { openWizard } from "./documentWizard.js";

let search = "";

export async function render(root) {
  setHeader({
    title: "Детализация заказов",
    sub: "Документы для покупателя: детализация заказа и чек «Мой налог»",
    actions: el(
      "button",
      {
        class: "btn btn--primary",
        onclick: async () => {
          await openWizard({
            onSaved: () => refresh(),
          });
        },
      },
      el("span", { html: icon("plus") }),
      "Новая детализация",
    ),
  });

  root.innerHTML = "";
  const page = el("div", { class: "stack", style: { gap: "14px" } });
  const searchInput = el("input", {
    class: "input",
    type: "search",
    placeholder: "Номер или покупатель",
    value: search,
  });
  const statsEl = el("div", { class: "small muted" });
  const toolbar = el(
    "div",
    { class: "toolbar", style: { marginBottom: "0" } },
    el("div", { class: "toolbar__search" }, el("span", { html: icon("search") }), searchInput),
    el("div", { class: "toolbar__spacer" }),
    statsEl,
  );
  const list = el("div", { class: "docs" });
  page.append(toolbar, list);
  root.appendChild(page);

  searchInput.addEventListener(
    "input",
    debounce(() => {
      search = searchInput.value.trim();
      refresh();
    }, 240),
  );

  async function refresh() {
    list.innerHTML = "";
    statsEl.textContent = "";
    let docs = [];
    try {
      docs = await api.documents(search || null);
    } catch (e) {
      list.appendChild(
        el(
          "div",
          { class: "notice notice--danger" },
          el("div", { html: icon("alert") }),
          el("div", {}, String(e?.message ?? e)),
        ),
      );
      return;
    }

    const total = docs.reduce((s, d) => s + d.totalKop, 0);
    statsEl.textContent = `Документов: ${docs.length} · на сумму ${money(total)} ₽`;

    if (docs.length === 0) {
      list.appendChild(
        el(
          "div",
          { class: "card" },
          emptyState({
            iconName: "receipt",
            title: search ? "Ничего не найдено" : "Детализаций заказов пока нет",
            text: search
              ? "Попробуйте изменить запрос."
              : "Создайте первый документ: прикрепите чек «Мой налог» или отметьте, что пришлёте его покупателю позже.",
            action: search
              ? null
              : el(
                  "button",
                  {
                    class: "btn btn--primary btn--sm",
                    onclick: async () => {
                      await openWizard({ onSaved: () => refresh() });
                    },
                  },
                  "Создать детализацию заказа",
                ),
          }),
        ),
      );
      return;
    }

    let currentMonth = "";
    let group = null;
    for (const doc of docs) {
      const key = doc.docDate.slice(0, 7);
      if (key !== currentMonth) {
        currentMonth = key;
        const [year, month] = key.split("-").map(Number);
        list.appendChild(
          el(
            "div",
            { class: "docs__month" },
            el("span", {}, monthLabel(year, month)),
            el("span", { class: "small muted" }, `${docs.filter((d) => d.docDate.startsWith(key)).length} шт.`),
          ),
        );
        group = el("div", { class: "docs__group" });
        list.appendChild(group);
      }
      group.appendChild(docRow(doc));
    }
  }

  function docRow(doc) {
    const thumb = el("div", {
      class: "doc-row__thumb",
      style: { display: "grid", placeItems: "center", color: "var(--muted)" },
      html: icon("receipt"),
    });

    if (doc.receiptFileId) {
      api
        .receiptThumb(doc.receiptFileId)
        .then((url) => {
          thumb.innerHTML = "";
          thumb.style.display = "block";
          const img = el("img", {
            src: url,
            alt: "",
            style: { width: "100%", height: "100%", objectFit: "cover", borderRadius: "3px" },
          });
          thumb.appendChild(img);
        })
        .catch(() => {});
    }

    const sum = `${money(doc.totalKop)} ₽`;

    return el(
      "div",
      { class: "doc-row" },
      thumb,
      el(
        "div",
        { class: "doc-row__main" },
        el(
          "div",
          { class: "doc-row__title" },
          el(
            "button",
            {
              class: "doc-row__number",
              style: { background: "none", border: "none", padding: "0", cursor: "pointer", color: "var(--text)" },
              title: "Открыть PDF",
              onclick: () => openPdf(doc),
            },
            doc.number ? `№ ${doc.number}` : "Без номера чека",
          ),
          el("span", { class: "badge badge--outline" }, dateRu(doc.docDate)),
          doc.items.length > 0
            ? el("span", { class: "badge" }, `${doc.items.length} ${itemsWord(doc.items.length)}`)
            : null,
          doc.receiptPending || !doc.receiptFileId
            ? el("span", { class: "badge badge--warning" }, "Чек ожидается")
            : null,
        ),
        el(
          "div",
          { class: "doc-row__meta" },
          el("span", {}, doc.buyerName ? shortName(doc.buyerName, 46) : "Покупатель не указан"),
          doc.buyerInn ? el("span", {}, `ИНН ${doc.buyerInn}`) : null,
          doc.note ? el("span", {}, shortName(doc.note, 40)) : null,
        ),
      ),
      el("div", { class: "doc-row__sum" }, sum),
      el(
        "div",
        { class: "doc-row__actions" },
        el(
          "button",
          {
            class: "btn btn--secondary btn--sm",
            title: "Открыть PDF",
            onclick: () => openPdf(doc),
          },
          el("span", { html: icon("print") }),
          "PDF",
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Повторить документ",
            onclick: async () => {
              await openWizard({
                repeat: {
                  buyerName: doc.buyerName,
                  buyerInn: doc.buyerInn,
                  note: doc.note,
                  items: doc.items,
                },
                onSaved: () => refresh(),
              });
            },
          },
          el("span", { html: icon("copy") }),
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Изменить",
            onclick: async () => {
              const full = await api.document(doc.id).catch((e) => {
                toastError(e);
                return null;
              });
              if (!full) return;
              await openWizard({ document: full, onSaved: () => refresh() });
            },
          },
          el("span", { html: icon("edit") }),
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Удалить",
            onclick: async () => {
              const ok = await confirmDialog({
                title: "Удалить детализацию заказа?",
                text: doc.number
                  ? `Документ № ${doc.number} от ${dateRu(doc.docDate)} будет удалён. Чек НПД останется в базе.`
                  : `Детализация заказа от ${dateRu(doc.docDate)} будет удалена. Чек НПД останется в базе.`,
                confirmText: "Удалить",
                danger: true,
              });
              if (!ok) return;
              try {
                await api.deleteDocument(doc.id);
                toast("Документ удалён", { type: "success" });
                refresh();
              } catch (e) {
                toastError(e);
              }
            },
          },
          el("span", { html: icon("trash") }),
        ),
      ),
    );
  }

  async function openPdf(doc) {
    try {
      const path = await api.documentPdf(doc.id);
      await api.openPath(path);
      toast("PDF открыт в системном просмотрщике", {
        type: "success",
        timeout: 6000,
        actions: [{ label: "Показать в папке", onClick: () => api.revealPath(path) }],
      });
    } catch (e) {
      toastError(e);
    }
  }

  await refresh();
}
