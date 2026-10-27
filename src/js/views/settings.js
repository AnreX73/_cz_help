import { api } from "../api.js";
import {
  el,
  icon,
  field,
  toast,
  toastError,
} from "../ui.js";
import { money, parseMoney } from "../format.js";
import {
  setHeader,
  state,
  applyTheme,
  updateSidebar,
  refreshSellers,
} from "../shell.js";
import { openSellerForm, switchToSeller } from "./sellers.js";

let draft = null;
let dirty = false;

export async function render(root) {
  try {
    await refreshSellers();
  } catch {
    /* покажем то, что уже загружено при старте */
  }
  draft = { ...state.settings };
  dirty = false;

  const saveBtn = el(
    "button",
    { class: "btn btn--primary" },
    el("span", { html: icon("check") }),
    "Сохранить",
  );

  setHeader({
    title: "Настройки",
    sub: "Самозанятые, ставки, печать и данные",
    actions: saveBtn,
  });

  root.innerHTML = "";

  /* ---------- поля ---------- */

  const rateIndInput = textInput(rateText(draft.rateIndividual), "4");
  rateIndInput.classList.add("input--num");
  const rateOrgInput = textInput(rateText(draft.rateOrganization), "6");
  rateOrgInput.classList.add("input--num");

  const deductionInput = textInput(money(draft.deductionKop), "10 000,00");
  deductionInput.classList.add("input--num");
  const limitInput = textInput(money(draft.limitKop), "2 400 000,00");
  limitInput.classList.add("input--num");

  const widthInput = textInput(String(draft.receiptWidthMm), "70");
  widthInput.classList.add("input--num");
  widthInput.inputMode = "numeric";
  const unitInput = textInput(draft.defaultUnit, "усл.");
  const extraQrInput = el("input", { type: "checkbox", checked: draft.extraQr });
  const docsDirInput = textInput(draft.documentsDir, "По умолчанию: Документы\\Чеки");

  let theme = draft.theme || "light";

  const themePicker = el("div", { class: "theme-picker" });
  const themeOptions = [
    ["light", "Светлая", ["#f4f6f9", "#ffffff", "#3d63dd"]],
    ["dark", "Тёмная", ["#0d1117", "#151a22", "#7492ff"]],
    ["system", "Системная", ["#f4f6f9", "#0d1117", "#8d9bb0"]],
  ];
  const themeButtons = [];
  for (const [value, label, colors] of themeOptions) {
    const btn = el(
      "div",
      { class: `theme-option${value === theme ? " is-active" : ""}`, role: "button", tabindex: "0" },
      el(
        "div",
        { class: "theme-swatch" },
        el("span", { style: { background: colors[0] } }),
        el("span", { style: { background: colors[1] } }),
        el("span", { style: { background: colors[2] } }),
      ),
      el("span", {}, label),
    );
    const choose = () => {
      theme = value;
      themeButtons.forEach((b) => b.classList.remove("is-active"));
      btn.classList.add("is-active");
      applyTheme(value);
      draft.theme = value;
      markDirty();
    };
    btn.addEventListener("click", choose);
    btn.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        choose();
      }
    });
    themeButtons.push(btn);
    themePicker.appendChild(btn);
  }

  const headerImportant = el("span", { class: "badge badge--warning", style: { display: "none" } }, "не сохранено");

  /* ---------- самозанятые ---------- */

  const sellerList = el("div", { class: "seller-list" });

  function renderSellerList() {
    sellerList.innerHTML = "";
    for (const seller of state.sellers) {
      const active = state.seller?.id === seller.id;
      sellerList.appendChild(
        el(
          "div",
          {
            class: `seller-row${active ? " is-active" : ""}${seller.archived ? " is-archived" : ""}`,
          },
          el(
            "button",
            {
              class: "seller-row__main",
              type: "button",
              title: seller.archived ? "Профиль в архиве" : active ? "Активный профиль" : "Сделать активным",
              onclick: async () => {
                if (seller.archived) {
                  openSellerForm({ seller, onSaved: renderSellerList });
                  return;
                }
                if (active) return;
                try {
                  await switchToSeller(seller.id);
                  renderSellerList();
                } catch (e) {
                  toastError(e);
                }
              },
            },
            el(
              "span",
              { class: "seller-row__name" },
              seller.name || "Без имени",
              seller.archived ? el("span", { class: "badge badge--outline" }, "архив") : null,
            ),
            el("span", { class: "seller-row__meta" }, seller.inn ? `ИНН ${seller.inn}` : "ИНН не заполнен"),
          ),
          el(
            "span",
            { class: "seller-row__actions" },
            active ? el("span", { class: "seller-row__active" }, el("span", { html: icon("check") }), "активен") : null,
            el(
              "button",
              {
                class: "btn btn--ghost btn--icon",
                title: "Изменить",
                onclick: () => openSellerForm({ seller, onSaved: renderSellerList }),
              },
              el("span", { html: icon("edit") }),
            ),
          ),
        ),
      );
    }
    if (state.sellers.length === 0) {
      sellerList.appendChild(
        el(
          "div",
          { class: "notice notice--warning" },
          el("div", { html: icon("alert") }),
          el("div", {}, "Добавьте самозанятого, чтобы считать налог и печатать чеки."),
        ),
      );
    }
  }

  renderSellerList();

  const sellersCard = section(
    "Самозанятые",
    "Доходы, налог, вычет и лимит считаются отдельно по каждому профилю. Клиенты и услуги — общие",
    sellerList,
    el(
      "button",
      {
        class: "btn btn--secondary btn--sm",
        style: { alignSelf: "flex-start" },
        onclick: () => openSellerForm({ onSaved: renderSellerList }),
      },
      el("span", { html: icon("plus") }),
      "Добавить самозанятого",
    ),
  );

  const taxCard = section(
    "Ставки и лимиты",
    "По умолчанию: 4 % с физлиц, 6 % с юрлиц, вычет 10 000 ₽, лимит 2,4 млн ₽",
    el(
      "div",
      { class: "settings-row settings-row--3" },
      field(
        "Ставка с физлиц",
        el("div", { class: "input-group" }, rateIndInput, el("span", { class: "input-suffix" }, "%")),
      ),
      field(
        "Ставка с юрлиц",
        el("div", { class: "input-group" }, rateOrgInput, el("span", { class: "input-suffix" }, "%")),
      ),
      field(
        "Ширина чека НПД",
        el("div", { class: "input-group" }, widthInput, el("span", { class: "input-suffix" }, "мм")),
        "В блоке PDF",
      ),
    ),
    el(
      "div",
      { class: "settings-row" },
      field(
        "Налоговый вычет",
        el("div", { class: "input-group" }, deductionInput, el("span", { class: "input-suffix" }, "₽")),
      ),
      field(
        "Лимит дохода в год",
        el("div", { class: "input-group" }, limitInput, el("span", { class: "input-suffix" }, "₽")),
      ),
    ),
    el(
      "label",
      { class: "row", style: { gap: "8px", cursor: "pointer" } },
      extraQrInput,
      el(
        "span",
        {},
        el("div", { style: { fontSize: "13px", fontWeight: "500" } }, "Добавлять резервный QR-код в PDF"),
        el("div", { class: "field__hint" }, "Ссылка на чек ФНС рядом с оригинальным PNG"),
      ),
    ),
  );

  const printCard = section(
    "Печать документов",
    "Как собирать товарный чек",
    el(
      "div",
      { class: "settings-row" },
      field("Единица по умолчанию", unitInput),
      field("Папка для PDF", el("div", { class: "row", style: { gap: "6px" } }, docsDirInput, chooseDirBtn(docsDirInput))),
    ),
    el(
      "div",
      { class: "notice notice--info" },
      el("div", { html: icon("info") }),
      el(
        "div",
        {},
        "PDF сохраняется автоматически: ",
        el("code", {}, "Документы\\Чеки\\ГГГГ-ММ\\ДАТА_НОМЕР_ПОКУПАТЕЛЬ.pdf"),
      ),
    ),
  );

  const themeCard = section(
    "Внешний вид",
    "Тема применяется сразу",
    themePicker,
  );

  const main = el("div", { class: "settings__main" }, sellersCard, taxCard, printCard, themeCard);

  /* ---------- боковая колонка ---------- */

  const paths = state.paths ?? {};
  const pathsCard = el(
    "div",
    { class: "card" },
    el(
      "div",
      { class: "card__head" },
      el("h3", { class: "card__title" }, "Данные"),
      el("span", { class: "card__sub" }, "локально"),
    ),
    el(
      "div",
      { class: "card__body stack", style: { gap: "10px" } },
      pathRow("База данных", paths.dbPath),
      pathRow("Вложения", paths.attachmentsDir),
      pathRow("PDF-документы", draft.documentsDir || paths.documentsDir),
      el(
        "div",
        { class: "settings-actions" },
        el(
          "button",
          {
            class: "btn btn--secondary btn--sm",
            onclick: () => paths.dataDir && api.revealPath(paths.dataDir),
          },
          el("span", { html: icon("folder") }),
          "Открыть папку данных",
        ),
      ),
    ),
  );

  const backupCard = el(
    "div",
    { class: "card" },
    el(
      "div",
      { class: "card__head" },
      el("h3", { class: "card__title" }, "Резервная копия"),
    ),
    el(
      "div",
      { class: "card__body stack", style: { gap: "10px" } },
      el(
        "div",
        { class: "small text-2", style: { lineHeight: "1.5" } },
        "Сохраняет базу и все прикреплённые чеки в выбранную папку. Копию можно делать вручную в любое время.",
      ),
      el(
        "button",
        { class: "btn btn--secondary btn--sm", onclick: createBackup },
        el("span", { html: icon("download") }),
        "Создать копию…",
      ),
    ),
  );

  const aboutCard = el(
    "div",
    { class: "card" },
    el("div", { class: "card__head" }, el("h3", { class: "card__title" }, "О программе")),
    el(
      "div",
      { class: "card__body about" },
      el("div", {}, el("strong", {}, "Помощник самозанятого")),
      el("div", {}, "Версия 0.1.0 · Tauri 2, Rust, SQLite"),
      el(
        "div",
        {},
        "Данные хранятся только на этом компьютере. В интернет приложение не обращается: QR-код чека НПД ведёт на сайт ФНС при сканировании.",
      ),
      el(
        "div",
        {},
        "Налог и вычет рассчитываются справочно. Сверяйтесь с «Мой налог».",
      ),
    ),
  );

  const side = el("div", { class: "settings__side" }, pathsCard, backupCard, aboutCard);

  const actions = el(
    "div",
    { class: "settings-actions" },
    el(
      "button",
      { class: "btn btn--primary", onclick: () => save() },
      el("span", { html: icon("check") }),
      "Сохранить настройки",
    ),
    el(
      "button",
      {
        class: "btn btn--secondary",
        onclick: () => {
          draft = { ...state.settings };
          dirty = false;
          render(root);
        },
      },
      "Сбросить",
    ),
    headerImportant,
  );

  const page = el("div", { class: "settings" }, el("div", { class: "settings__main" }, main, actions), side);
  root.appendChild(page);
  /* ---------- помощники ---------- */

  function textInput(value, placeholder) {
    const node = el("input", { class: "input", value: value ?? "", placeholder });
    node.addEventListener("input", markDirty);
    return node;
  }

  function markDirty() {
    dirty = true;
    headerImportant.style.display = "";
  }

  function chooseDirBtn(target) {
    return el(
      "button",
      {
        class: "btn btn--secondary",
        style: { flex: "none" },
        onclick: async () => {
          try {
            const { open } = await import("@tauri-apps/plugin-dialog");
            const selected = await open({ title: "Папка для PDF", directory: true, multiple: false });
            if (typeof selected === "string") {
              target.value = selected;
              markDirty();
            }
          } catch (e) {
            toastError(e);
          }
        },
      },
      el("span", { html: icon("folder") }),
      "Выбрать",
    );
  }

  async function createBackup() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const dir = await open({ title: "Куда сохранить резервную копию", directory: true, multiple: false });
      if (typeof dir !== "string") return;
      const path = await api.backup(dir);
      toast("Резервная копия создана", {
        type: "success",
        title: path,
        timeout: 9000,
        actions: [{ label: "Открыть папку", onClick: () => api.revealPath(path) }],
      });
    } catch (e) {
      toastError(e);
    }
  }

  async function save() {
    const rateInd = parseRate(rateIndInput.value);
    const rateOrg = parseRate(rateOrgInput.value);
    if (rateInd === null || rateInd <= 0 || rateInd > 1000) {
      toast("Ставка с физлиц — от 0,01 до 10 %", { type: "error" });
      rateIndInput.focus();
      return;
    }
    if (rateOrg === null || rateOrg <= 0 || rateOrg > 1000) {
      toast("Ставка с юрлиц — от 0,01 до 10 %", { type: "error" });
      rateOrgInput.focus();
      return;
    }
    const deduction = parseMoney(deductionInput.value);
    if (deduction === null || deduction < 0) {
      toast("Укажите корректный вычет", { type: "error" });
      deductionInput.focus();
      return;
    }
    const limit = parseMoney(limitInput.value);
    if (limit === null || limit <= 0) {
      toast("Укажите корректный лимит", { type: "error" });
      limitInput.focus();
      return;
    }
    const width = Number(String(widthInput.value).replace(",", ".")) || 70;
    if (width < 40 || width > 100) {
      toast("Ширина чека — от 40 до 100 мм", { type: "error" });
      widthInput.focus();
      return;
    }

    const payload = {
      ...draft,
      rateIndividual: rateInd,
      rateOrganization: rateOrg,
      deductionKop: deduction,
      limitKop: limit,
      receiptWidthMm: Math.round(width),
      extraQr: extraQrInput.checked,
      theme,
      defaultUnit: unitInput.value.trim() || "усл.",
      documentsDir: docsDirInput.value.trim(),
    };

    saveBtn.disabled = true;
    try {
      state.settings = await api.saveSettings(payload);
      draft = { ...state.settings };
      applyTheme(state.settings.theme);
      updateSidebar();
      dirty = false;
      headerImportant.style.display = "none";
      toast("Настройки сохранены", { type: "success", timeout: 2600 });
    } catch (e) {
      toastError(e);
    } finally {
      saveBtn.disabled = false;
    }
  }

  saveBtn.addEventListener("click", save);
}

function section(title, sub, ...children) {
  return el(
    "div",
    { class: "card" },
    el(
      "div",
      { class: "card__head" },
      el("div", {}, el("h3", { class: "card__title" }, title), sub ? el("div", { class: "card__sub" }, sub) : null),
    ),
    el("div", { class: "card__body stack", style: { gap: "12px" } }, ...children),
  );
}

function pathRow(label, value) {
  return el(
    "div",
    { class: "stack", style: { gap: "3px" } },
    el("div", { class: "field__label" }, label),
    el("div", { class: "path-box" }, value || "—"),
  );
}

function parseRate(value) {
  const num = Number(String(value).trim().replace(",", "."));
  if (!Number.isFinite(num)) return null;
  return Math.round(num * 100);
}

function rateText(hundredths) {
  const value = (hundredths ?? 0) / 100;
  return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(".", ",");
}
