import { api } from "../api.js";
import { el, icon, emptyState, toast, toastError } from "../ui.js";
import {
  money,
  rub,
  moneyCompact,
  monthLabel,
  monthShort,
  entriesWord,
} from "../format.js";
import { setHeader, navigate } from "../shell.js";

let year = new Date().getFullYear();

export async function render(root) {
  root.innerHTML = "";
  const page = el("div", { class: "stack", style: { gap: "16px" } });
  root.appendChild(page);

  const yearValue = el("div", { class: "year-switch__value" }, String(year));
  const prevBtn = el(
    "button",
    { class: "btn btn--secondary btn--icon btn--sm", title: "Предыдущий год" },
    el("span", { html: icon("left") }),
  );
  const nextBtn = el(
    "button",
    { class: "btn btn--secondary btn--icon btn--sm", title: "Следующий год" },
    el("span", { html: icon("right") }),
  );
  const exportBtn = el(
    "button",
    { class: "btn btn--secondary" },
    el("span", { html: icon("download") }),
    "CSV за год",
  );

  setHeader({
    title: "Отчёты",
    sub: "Доходы, налог и вычет по месяцам",
    actions: el(
      "div",
      { class: "row", style: { gap: "10px" } },
      el("div", { class: "year-switch" }, prevBtn, yearValue, nextBtn),
      exportBtn,
    ),
  });

  prevBtn.addEventListener("click", () => {
    year -= 1;
    yearValue.textContent = String(year);
    load();
  });
  nextBtn.addEventListener("click", () => {
    year += 1;
    yearValue.textContent = String(year);
    load();
  });

  exportBtn.addEventListener("click", async () => {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const path = await save({
        title: `Экспорт доходов за ${year} год`,
        defaultPath: `Доходы_${year}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return;
      await api.exportCsv(path, `${year}-01-01`, `${year}-12-31`);
      toast("Отчёт сохранён", {
        type: "success",
        title: path,
        actions: [{ label: "Открыть папку", onClick: () => api.revealPath(path) }],
      });
    } catch (e) {
      toastError(e);
    }
  });

  async function load() {
    page.innerHTML = "";
    page.appendChild(
      el(
        "div",
        { class: "empty", style: { paddingTop: "60px" } },
        el("div", { class: "empty__icon", html: icon("refresh", "spin") }),
        el("div", { class: "empty__text" }, "Считаем…"),
      ),
    );

    let months = [];
    try {
      months = await api.months();
    } catch (e) {
      page.innerHTML = "";
      page.appendChild(
        el(
          "div",
          { class: "notice notice--danger" },
          el("div", { html: icon("alert") }),
          el("div", {}, String(e?.message ?? e)),
        ),
      );
      return;
    }

    const ofYear = months.filter((m) => m.year === year);
    const byMonth = new Map(ofYear.map((m) => [m.month, m]));
    const income = ofYear.reduce((s, m) => s + m.incomeTotalKop, 0);
    const tax = ofYear.reduce((s, m) => s + m.taxKop, 0);
    const deduction = ofYear.reduce((s, m) => s + m.deductionUsedKop, 0);
    const count = ofYear.reduce((s, m) => s + m.entriesCount, 0);

    page.innerHTML = "";

    if (ofYear.length === 0) {
      page.appendChild(
        el(
          "div",
          { class: "card" },
          emptyState({
            iconName: "chart",
            title: `За ${year} год данных нет`,
            text: "Добавьте доходы или выберите другой год.",
            action: el(
              "button",
              { class: "btn btn--primary btn--sm", onclick: () => navigate("income") },
              "Перейти к доходам",
            ),
          }),
        ),
      );
      return;
    }

    page.appendChild(
      el(
        "div",
        { class: "grid grid--4" },
        statCard("Доход за год", `${money(income)} ₽`, `${count} ${entriesWord(count)}`, "wallet"),
        statCard("Налог за год", `${money(tax)} ₽`, "4 % / 6 % с учётом вычета", "receipt", "accent"),
        statCard("Вычет использован", `${money(deduction)} ₽`, "Из 10 000 ₽", "sparkles"),
        statCard(
          "Средний доход в месяц",
          `${money(Math.round(income / Math.max(ofYear.length, 1)))} ₽`,
          `${ofYear.length} мес. с доходом`,
          "chart",
        ),
      ),
    );

    const max = Math.max(...ofYear.map((m) => m.incomeTotalKop), 1);
    const chart = el(
      "div",
      { class: "card" },
      el(
        "div",
        { class: "card__head" },
        el("h3", { class: "card__title" }, "Доход по месяцам"),
        el("span", { class: "card__sub" }, `${year} год`),
        el(
          "div",
          { class: "card__actions" },
          el(
            "button",
            {
              class: "btn btn--ghost btn--sm",
              onclick: () => {
                year = new Date().getFullYear();
                yearValue.textContent = String(year);
                load();
              },
            },
            "Текущий год",
          ),
        ),
      ),
      el(
        "div",
        { class: "card__body" },
        el(
          "div",
          { class: "bars" },
          ...Array.from({ length: 12 }, (_, idx) => {
            const month = idx + 1;
            const data = byMonth.get(month);
            const value = data?.incomeTotalKop ?? 0;
            const pct = value > 0 ? Math.max((value / max) * 100, 4) : 0;
            const isCurrent = year === new Date().getFullYear() && month === new Date().getMonth() + 1;
            return el(
              "div",
              { class: "bar-col", title: `${monthLabel(year, month)}: ${rub(value)}` },
              el("div", { class: "bar-col__value" }, value > 0 ? moneyCompact(value) : ""),
              el(
                "div",
                { class: "bar-col__track", style: { flex: "1", height: "auto" } },
                el("div", {
                  class: `bar-col__bar${isCurrent ? " is-current" : ""}${value === 0 ? " is-empty" : ""}`,
                  style: { height: value > 0 ? `${pct}%` : "2px" },
                }),
              ),
              el("div", { class: "bar-col__label" }, monthShort(month)),
            );
          }),
        ),
      ),
    );
    page.appendChild(chart);

    /* кварталы */
    const quarters = el("div", { class: "grid grid--4" });
    const names = ["I квартал", "II квартал", "III квартал", "IV квартал"];
    for (let q = 0; q < 4; q += 1) {
      const qMonths = ofYear.filter((m) => m.month >= q * 3 + 1 && m.month <= q * 3 + 3);
      const qIncome = qMonths.reduce((s, m) => s + m.incomeTotalKop, 0);
      const qTax = qMonths.reduce((s, m) => s + m.taxKop, 0);
      quarters.appendChild(
        el(
          "div",
          { class: "card card--pad" },
          el("div", { class: "card__sub" }, names[q]),
          el("div", { style: { fontSize: "18px", fontWeight: "600", letterSpacing: "-0.02em", marginTop: "2px" } }, `${money(qIncome)} ₽`),
          el("div", { class: "small muted", style: { marginTop: "2px" } }, `налог ${money(qTax)} ₽`),
        ),
      );
    }
    page.appendChild(quarters);

    /* таблица */
    const tableCard = el(
      "div",
      { class: "card" },
      el(
        "div",
        { class: "card__head" },
        el("h3", { class: "card__title" }, "Разбивка по месяцам"),
        el("span", { class: "card__sub" }, "Вычет расходуется хронологически"),
      ),
    );

    const table = el(
      "table",
      { class: "table" },
      el(
        "thead",
        {},
        el(
          "tr",
          {},
          el("th", {}, "Месяц"),
          el("th", { class: "col-num" }, "Физлица"),
          el("th", { class: "col-num" }, "Юрлица"),
          el("th", { class: "col-num" }, "Доход"),
          el("th", { class: "col-num" }, "Налог"),
          el("th", { class: "col-num" }, "Вычет"),
          el("th", { class: "col-num" }, "Операций"),
        ),
      ),
    );
    const tbody = el("tbody");
    for (let month = 1; month <= 12; month += 1) {
      const data = byMonth.get(month);
      const empty = !data;
      tbody.appendChild(
        el(
          "tr",
          {},
          el("td", { class: empty ? "cell-muted" : "cell-strong" }, monthLabel(year, month)),
          el("td", { class: "col-num" }, empty ? "—" : `${money(data.incomeIndividualKop)}`),
          el("td", { class: "col-num" }, empty ? "—" : `${money(data.incomeOrganizationKop)}`),
          el("td", { class: "col-num cell-strong" }, empty ? "—" : `${money(data.incomeTotalKop)}`),
          el("td", { class: "col-num" }, empty ? "—" : `${money(data.taxKop)}`),
          el("td", { class: "col-num cell-muted" }, empty ? "—" : `${money(data.deductionUsedKop)}`),
          el("td", { class: "col-num cell-muted" }, empty ? "—" : String(data.entriesCount)),
        ),
      );
    }
    table.appendChild(tbody);
    tableCard.appendChild(el("div", { class: "table-wrap" }, table));
    tableCard.appendChild(
      el(
        "div",
        { class: "table-foot" },
        el("span", {}, "Доход: ", el("strong", {}, `${money(income)} ₽`)),
        el("span", {}, "Налог: ", el("strong", {}, `${money(tax)} ₽`)),
        el("span", {}, "Вычет: ", el("strong", {}, `${money(deduction)} ₽`)),
      ),
    );
    page.appendChild(tableCard);

    page.appendChild(
      el(
        "div",
        { class: "small muted", style: { lineHeight: "1.5" } },
        "Налог рассчитывается по каждой операции: 4 % с доходов от физлиц и 6 % с доходов от юрлиц. Вычет 10 000 ₽ уменьшает разницу между 6 % и 4 % и расходуется с начала деятельности. Данные носят справочный характер — сверяйтесь с приложением «Мой налог».",
      ),
    );
  }

  await load();
}

function statCard(label, value, hint, iconName, variant = "") {
  return el(
    "div",
    { class: `kpi ${variant ? `kpi--${variant}` : ""}` },
    el(
      "div",
      { class: "kpi__label" },
      el("span", { html: icon(iconName), style: { width: "14px", height: "14px" } }),
      label,
    ),
    el("div", { class: "kpi__value", style: { fontSize: "22px" } }, value),
    el("div", { class: "kpi__foot" }, hint),
  );
}
