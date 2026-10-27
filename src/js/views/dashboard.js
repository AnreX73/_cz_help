import { api } from "../api.js";
import { el, icon, toast, toastError, emptyState } from "../ui.js";
import {
  money,
  rub,
  dateFull,
  todayIso,
  daysUntil,
  monthLabel,
  monthLabelLower,
  entriesWord,
  payerLabel,
  plural,
} from "../format.js";
import { openEntryModal } from "./entryModal.js";
import { openWizard } from "./documentWizard.js";
import { navigate, setHeader, state } from "../shell.js";

export async function render(root) {
  setHeader({
    title: "Обзор",
    sub: dateFull(todayIso()),
    actions: el(
      "button",
      { class: "btn btn--primary", onclick: () => openEntryModal({ onSaved: () => render(root) }) },
      el("span", { html: icon("plus") }),
      "Новый доход",
    ),
  });

  let d;
  try {
    d = await api.dashboard();
  } catch (e) {
    toastError(e);
    return;
  }
  if (!root.isConnected) return;
  root.innerHTML = "";

  const wrap = el("div", { class: "dash" });

  const seller = d.seller;
  if (!seller?.name || !seller?.inn) {
    const text = !seller?.name
      ? "Профиль не заполнен: укажите имя и ИНН — они попадут в детализацию заказа. "
      : "У профиля не заполнен ИНН — без него нельзя сохранить детализацию заказа. ";
    wrap.appendChild(
      el(
        "div",
        { class: "notice notice--warning" },
        el("div", { html: icon("alert") }),
        el(
          "div",
          {},
          `${seller?.name ? `«${seller.name}»: ` : ""}${text}`,
          el(
            "a",
            {
              href: "#/settings",
              style: { color: "inherit", fontWeight: "600", textDecoration: "underline" },
            },
            "Открыть настройки",
          ),
        ),
      ),
    );
  }

  if (d.due) {
    const days = daysUntil(d.due.dueDate);
    const overdue = days < 0 && !d.due.paid;
    const soon = days >= 0 && days <= 5 && !d.due.paid;
    const cls = d.due.paid ? "due due--ok" : overdue || soon ? "due due--warn" : "due";
    const dateLabel = dateFull(d.due.dueDate);
    const label = d.due.paid
      ? `Налог за ${monthLabelLower(d.due.year, d.due.month)} уплачен`
      : `Налог за ${monthLabelLower(d.due.year, d.due.month)}`;
    const hint = d.due.paid
      ? "Отметка сохранена локально"
      : overdue
        ? `Срок прошёл ${Math.abs(days)} ${plural(Math.abs(days), "день", "дня", "дней")} назад — проверьте «Мой налог»`
        : `Уплатить до ${dateLabel} · осталось ${days} ${plural(days, "день", "дня", "дней")}`;

    const toggle = el(
      "button",
      {
        class: `btn btn--sm ${d.due.paid ? "btn--secondary" : "btn--primary"}`,
        onclick: async () => {
          try {
            await api.setPaid(d.due.year, d.due.month, !d.due.paid);
            render(root);
          } catch (e) {
            toastError(e);
          }
        },
      },
      el("span", { html: icon(d.due.paid ? "refresh" : "check") }),
      d.due.paid ? "Снять отметку" : "Отметить оплату",
    );

    wrap.appendChild(
      el(
        "div",
        { class: cls },
        el("span", { html: icon(d.due.paid ? "checkCircle" : "alert"), style: { color: d.due.paid ? "var(--success)" : "var(--warning)" } }),
        el(
          "div",
          { class: "due__main" },
          el("div", { class: "due__label" }, label),
          el("div", { class: "due__value" }, rub(d.due.taxKop)),
          el("div", { class: "due__date" }, hint),
        ),
        toggle,
      ),
    );
  }

  const kpis = el("div", { class: "dash__kpis" });
  kpis.append(
    kpiCard(
      `Доход за ${monthLabel(...currentYearMonth()).split(" ")[0].toLowerCase()}`,
      d.month.incomeKop,
      `${d.month.entriesCount} ${entriesWord(d.month.entriesCount)} · налог ${rub(d.month.taxKop)}`,
      "wallet",
    ),
    kpiCard(
      "Налог за месяц",
      d.month.taxKop,
      `Вычет израсходован на ${rub(d.deductionTotalKop - d.deductionLeftKop)}`,
      "receipt",
      "accent",
    ),
    kpiCard(
      `Доход за ${currentYear()} год`,
      d.year.incomeKop,
      `Квартал: ${rub(d.quarter.incomeKop)} · налог ${rub(d.quarter.taxKop)}`,
      "chart",
    ),
  );
  wrap.appendChild(kpis);

  const grid = el("div", { class: "dash__grid" });

  const recentCard = el(
    "div",
    { class: "card" },
    el(
      "div",
      { class: "card__head" },
      el("h3", { class: "card__title" }, "Последние операции"),
      el("span", { class: "card__sub" }, `${entriesWord(d.recent.length)}`),
      el(
        "div",
        { class: "card__actions" },
        el("a", { class: "btn btn--ghost btn--sm", href: "#/income" }, "Все доходы", el("span", { html: icon("right") })),
      ),
    ),
  );

  if (d.recent.length === 0) {
    recentCard.appendChild(
      emptyState({
        iconName: "wallet",
        title: "Пока нет доходов",
        text: "Добавьте первую операцию — налог и лимит посчитаются сами.",
        action: el(
          "button",
          { class: "btn btn--primary btn--sm", onclick: () => openEntryModal({ onSaved: () => render(root) }) },
          "Добавить доход",
        ),
      }),
    );
  } else {
    const list = el("div", { class: "list" });
    for (const e of d.recent) {
      list.appendChild(
        el(
          "div",
          { class: "list__item" },
          el(
            "div",
            { class: "list__main" },
            el("div", { class: "list__title" }, e.description || e.clientName || "Доход"),
            el(
              "div",
              { class: "list__sub" },
              `${e.date.slice(8, 10)}.${e.date.slice(5, 7)}.${e.date.slice(0, 4)} · ${payerLabel(e.payerType)}${
                e.receiptIssued ? " · чек выдан" : ""
              }`,
            ),
          ),
          el("div", { class: "list__value" }, rub(e.amountKop)),
        ),
      );
    }
    recentCard.appendChild(list);
  }
  grid.appendChild(recentCard);

  const side = el("div", { class: "dash__limits" });

  const pct = Math.round(d.limitRatio * 100);
  const limitClass = d.limitRatio >= 0.95 ? "progress__bar--danger" : d.limitRatio >= 0.8 ? "progress__bar--warning" : "";
  const limitCard = el(
    "div",
    { class: "card card--pad" },
    el(
      "div",
      { class: "limit-block" },
      el(
        "div",
        { class: "limit-block__top" },
        el("div", { class: "card__title" }, "Лимит дохода"),
        el("span", { class: `badge ${pct >= 80 ? "badge--warning" : "badge--accent"}` }, `${pct} %`),
      ),
      el("div", { class: "limit-block__value" }, rub(d.year.incomeKop)),
      el("div", { class: "progress" }, el("div", { class: `progress__bar ${limitClass}`, style: { width: `${Math.max(pct, 1.5)}%` } })),
      el(
        "div",
        { class: "limit-block__rest" },
        `Осталось ${rub(d.limitLeftKop)} из ${money(d.limitTotalKop)} ₽`,
      ),
      d.limitRatio >= 0.8
        ? el(
            "div",
            { class: "notice notice--warning", style: { marginTop: "6px" } },
            el("div", { html: icon("alert") }),
            el("div", {}, "Близко к лимиту: при превышении права на НПД прекращаются."),
          )
        : null,
    ),
  );

  const used = d.deductionTotalKop - d.deductionLeftKop;
  const dedPct = d.deductionTotalKop > 0 ? Math.round((used / d.deductionTotalKop) * 100) : 0;
  const deductionCard = el(
    "div",
    { class: "card card--pad" },
    el(
      "div",
      { class: "limit-block" },
      el(
        "div",
        { class: "limit-block__top" },
        el("div", { class: "card__title" }, "Налоговый вычет"),
        el("span", { class: "badge" }, `${dedPct} %`),
      ),
      el("div", { class: "limit-block__value" }, rub(d.deductionLeftKop)),
      el("div", { class: "progress" }, el("div", { class: "progress__bar", style: { width: `${Math.max(dedPct, 1)}%` } })),
      el("div", { class: "limit-block__rest" }, `Осталось из ${money(d.deductionTotalKop)} ₽`),
      el(
        "div",
        { class: "field__hint", style: { marginTop: "2px" } },
        "Вычет уменьшает налог с доходов от юрлиц: 6 % превращаются в 4 %, пока не израсходуется.",
      ),
    ),
  );

  const actionsCard = el(
    "div",
    { class: "card card--pad" },
    el("div", { class: "card__title", style: { marginBottom: "10px" } }, "Быстрые действия"),
    el(
      "div",
      { class: "quick" },
      quickButton("receipt", "Детализация заказа", "С чеком или без", () => openWizard({ onSaved: () => render(root) })),
      quickButton("plus", "Доход", "Новая операция", () => openEntryModal({ onSaved: () => render(root) })),
      quickButton("users", "Клиенты", "Справочник", () => navigate("clients")),
      quickButton("chart", "Отчёты", "Сводка за год", () => navigate("reports")),
    ),
    d.unissuedReceipts > 0
      ? el(
          "div",
          { class: "notice notice--warning", style: { marginTop: "10px" } },
          el("div", { html: icon("alert") }),
          el(
            "div",
            {},
            `${d.unissuedReceipts} ${plural(d.unissuedReceipts, "доход", "дохода", "доходов")} без чека НПД. `,
            el("a", { href: "#/income", style: { color: "inherit", fontWeight: "600" } }, "Проверить"),
          ),
        )
      : null,
  );

  side.append(limitCard, deductionCard, actionsCard);
  grid.appendChild(side);
  wrap.appendChild(grid);
  root.appendChild(wrap);
}

function currentYearMonth() {
  const now = new Date();
  return [now.getFullYear(), now.getMonth() + 1];
}

function currentYear() {
  return new Date().getFullYear();
}

function kpiCard(label, valueKop, foot, iconName, variant = "") {
  return el(
    "div",
    { class: `kpi ${variant ? `kpi--${variant}` : ""}` },
    el("div", { class: "kpi__label" }, el("span", { html: icon(iconName), style: { width: "14px", height: "14px" } }), label),
    el("div", { class: "kpi__value" }, money(valueKop), el("small", {}, "₽")),
    el("div", { class: "kpi__foot" }, foot),
  );
}

function quickButton(iconName, title, sub, onClick) {
  return el(
    "button",
    { class: "quick__btn", onclick: onClick },
    el("span", { html: icon(iconName) }),
    el("span", { class: "quick__text" }, el("div", { class: "quick__title" }, title), el("div", { class: "quick__sub" }, sub)),
  );
}
