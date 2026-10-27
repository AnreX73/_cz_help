import { api } from "../api.js";
import {
  el,
  icon,
  emptyState,
  toast,
  toastError,
  confirmDialog,
  debounce,
  openModal,
  field,
} from "../ui.js";
import { money, rub, parseMoney, plural } from "../format.js";
import { setHeader } from "../shell.js";

const NO_CATEGORY = "__none__";

let search = "";
let activeCategory = "";

export async function render(root) {
  const headerCategories = el(
    "button",
    {
      class: "btn btn--secondary",
      onclick: () => openCategoriesModal({ onSaved: () => refresh() }),
    },
    el("span", { html: icon("tag") }),
    "Категории",
  );
  const headerNew = el(
    "button",
    { class: "btn btn--primary", onclick: () => openServiceModal({ onSaved: () => refresh() }) },
    el("span", { html: icon("plus") }),
    "Новая услуга",
  );
  setHeader({
    title: "Услуги",
    sub: "Шаблоны позиций для товарных чеков",
    actions: [headerCategories, headerNew],
  });

  root.innerHTML = "";
  const page = el("div", { class: "stack", style: { gap: "14px" } });
  const searchInput = el("input", {
    class: "input",
    type: "search",
    placeholder: "Название или категория",
    value: search,
  });
  const countEl = el("div", { class: "small muted" });
  const toolbar = el(
    "div",
    { class: "toolbar", style: { marginBottom: "0" } },
    el("div", { class: "toolbar__search" }, el("span", { html: icon("search") }), searchInput),
    el("div", { class: "toolbar__spacer" }),
    countEl,
  );
  const chips = el("div", { class: "chip-row" });
  const card = el("div", { class: "card" });
  page.append(
    el(
      "div",
      { class: "notice notice--info" },
      el("div", { html: icon("info") }),
      el(
        "div",
        {},
        "Этот справочник подставляется в мастер товарного чека. Цены можно менять при каждом документе.",
      ),
    ),
    toolbar,
    chips,
    card,
  );
  root.appendChild(page);

  searchInput.addEventListener(
    "input",
    debounce(() => {
      search = searchInput.value.trim();
      refresh();
    }, 220),
  );

  async function refresh() {
    card.innerHTML = "";
    let services = [];
    let categories = [];
    try {
      [services, categories] = await Promise.all([api.services(), api.serviceCategories()]);
    } catch (e) {
      card.appendChild(
        el(
          "div",
          { class: "notice notice--danger", style: { margin: "16px" } },
          el("div", { html: icon("alert") }),
          el("div", {}, String(e?.message ?? e)),
        ),
      );
      return;
    }

    if (
      activeCategory &&
      activeCategory !== NO_CATEGORY &&
      !categories.some((c) => c.name.toLowerCase() === activeCategory.toLowerCase())
    ) {
      activeCategory = "";
    }

    const uncategorized = services.filter((s) => !s.category.trim()).length;
    if (activeCategory === NO_CATEGORY && uncategorized === 0) activeCategory = "";

    const q = search.toLowerCase();
    const visible = services.filter((s) => {
      if (q && !`${s.name} ${s.category} ${s.unit}`.toLowerCase().includes(q)) return false;
      const key = s.category.trim();
      if (!activeCategory) return true;
      if (activeCategory === NO_CATEGORY) return !key;
      return key.toLowerCase() === activeCategory.toLowerCase();
    });
    countEl.textContent = `${visible.length} из ${services.length}`;

    renderChips(categories, services, uncategorized);
    renderCard(visible, categories, q);
  }

  function renderChips(categories, services, uncategorized) {
    chips.innerHTML = "";
    if (categories.length === 0 && uncategorized === 0) {
      chips.hidden = true;
      return;
    }
    chips.hidden = false;
    const item = (label, value, count) =>
      el(
        "button",
        {
          class: `chip-toggle${activeCategory === value ? " is-active" : ""}`,
          onclick: () => {
            activeCategory = activeCategory === value ? "" : value;
            refresh();
          },
        },
        label,
        count === null ? null : el("span", { class: "chip-toggle__count" }, String(count)),
      );
    chips.appendChild(item("Все", "", services.length));
    for (const category of categories) {
      chips.appendChild(item(category.name, category.name, category.serviceCount));
    }
    if (uncategorized > 0) {
      chips.appendChild(item("Без категории", NO_CATEGORY, uncategorized));
    }
  }

  function renderCard(visible, categories, q) {
    card.appendChild(
      el(
        "div",
        { class: "card__head" },
        el("h3", { class: "card__title" }, "Каталог услуг"),
        el(
          "span",
          { class: "card__sub" },
          activeCategory === NO_CATEGORY
            ? "Без категории"
            : activeCategory || "Нажмите на строку, чтобы изменить",
        ),
        el(
          "div",
          { class: "card__actions" },
          el(
            "button",
            { class: "btn btn--ghost btn--sm", onclick: () => openServiceModal({ onSaved: () => refresh() }) },
            el("span", { html: icon("plus") }),
            "Добавить",
          ),
        ),
      ),
    );

    if (visible.length === 0) {
      card.appendChild(
        emptyState({
          iconName: "list",
          title: q ? "Ничего не найдено" : activeCategory ? "В категории пусто" : "Каталог пуст",
          text: q
            ? "Попробуйте другой запрос."
            : activeCategory
              ? "Выберите другую категорию или добавьте услугу."
              : "Добавьте услуги, которые чаще всего продаёте.",
          action: q
            ? null
            : el(
                "button",
                { class: "btn btn--primary btn--sm", onclick: () => openServiceModal({ onSaved: () => refresh() }) },
                "Добавить услугу",
              ),
        }),
      );
      return;
    }

    const order = new Map(categories.map((c, i) => [c.name.toLowerCase(), i]));
    const groups = new Map();
    for (const service of visible) {
      const key = service.category.trim();
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(service);
    }
    const sorted = [...groups.entries()].sort(([a], [b]) => {
      if (!a) return 1;
      if (!b) return -1;
      const ia = order.get(a.toLowerCase()) ?? Number.MAX_SAFE_INTEGER;
      const ib = order.get(b.toLowerCase()) ?? Number.MAX_SAFE_INTEGER;
      return ia - ib || a.localeCompare(b, "ru");
    });

    const table = el(
      "table",
      { class: "table" },
      el(
        "thead",
        {},
        el(
          "tr",
          {},
          el("th", {}, "Наименование"),
          el("th", { style: { width: "90px" } }, "Ед."),
          el("th", { class: "col-num", style: { width: "140px" } }, "Цена"),
          el("th", { class: "col-actions", style: { width: "76px" } }),
        ),
      ),
    );
    const tbody = el("tbody");
    for (const [category, items] of sorted) {
      tbody.appendChild(
        el(
          "tr",
          { class: "row--group" },
          el(
            "td",
            { colspan: "4" },
            el(
              "span",
              { class: "row--group__label" },
              category || "Без категории",
              el(
                "span",
                { class: "row--group__count" },
                `${items.length} ${plural(items.length, "услуга", "услуги", "услуг")}`,
              ),
            ),
          ),
        ),
      );
      for (const service of items) {
        tbody.appendChild(serviceRow(service));
      }
    }
    table.appendChild(tbody);
    card.appendChild(el("div", { class: "table-wrap" }, table));
  }

  function serviceRow(service) {
    const tr = el(
      "tr",
      { style: { cursor: "pointer" } },
      el("td", {}, el("span", { class: "cell-strong" }, service.name)),
      el("td", { class: "cell-muted" }, service.unit),
      el("td", { class: "col-num cell-strong" }, `${money(service.priceKop)} ₽`),
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
              openServiceModal({ service, onSaved: () => refresh() });
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
                title: "Удалить услугу?",
                text: `«${service.name}» исчезнет из каталога. В уже созданных чеках она останется.`,
                confirmText: "Удалить",
                danger: true,
              });
              if (!ok) return;
              try {
                await api.deleteService(service.id);
                toast("Услуга удалена", { type: "success" });
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
    tr.addEventListener("click", () => openServiceModal({ service, onSaved: () => refresh() }));
    return tr;
  }

  await refresh();
}

/* ---------- справочник категорий ---------- */

function openCategoriesModal({ onSaved = null } = {}) {
  let categories = [];
  const newInput = el("input", {
    class: "input",
    placeholder: "Например: Полиграфия",
    maxlength: "60",
  });
  const addButton = el(
    "button",
    { class: "btn btn--primary", onclick: () => addCategory() },
    el("span", { html: icon("plus") }),
    "Добавить",
  );
  const errorEl = el("div", { class: "modal__error" });
  const listEl = el("div", { class: "cat-list" });

  const modal = openModal({
    title: "Категории услуг",
    sub: "Общий справочник для всех самозанятых",
    body: el(
      "div",
      { class: "stack", style: { gap: "12px" } },
      el("div", { class: "cat-add" }, newInput, addButton),
      errorEl,
      listEl,
    ),
    footer: [
      el("div", { class: "small muted" }, "Категории нужны для группировки каталога"),
      el("div", { class: "spacer" }),
      el("button", { class: "btn btn--secondary", onclick: () => modal.close() }, "Готово"),
    ],
  });

  newInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addCategory();
    }
  });
  newInput.addEventListener("input", () => {
    if (errorEl.textContent) errorEl.textContent = "";
  });

  async function load() {
    try {
      categories = await api.serviceCategories();
    } catch (e) {
      errorEl.textContent = String(e?.message ?? e);
      return;
    }
    errorEl.textContent = "";
    renderList();
    onSaved?.();
  }

  function renderList() {
    listEl.innerHTML = "";
    if (categories.length === 0) {
      listEl.appendChild(
        emptyState({
          iconName: "tag",
          title: "Категорий пока нет",
          text: "Добавьте категорию — она появится в форме услуги и в фильтре.",
        }),
      );
      return;
    }
    for (const category of categories) {
      listEl.appendChild(categoryRow(category));
    }
  }

  function categoryRow(category) {
    const row = el(
      "div",
      { class: "cat-row" },
      el(
        "div",
        { class: "cat-row__main" },
        el("span", { class: "cat-row__name" }, category.name),
        el(
          "span",
          { class: "cat-row__count" },
          `${category.serviceCount} ${plural(category.serviceCount, "услуга", "услуги", "услуг")}`,
        ),
      ),
      el(
        "div",
        { class: "cat-row__actions" },
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Переименовать",
            onclick: () => startRename(row, category),
          },
          el("span", { html: icon("edit") }),
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Удалить",
            onclick: () => removeCategory(category),
          },
          el("span", { html: icon("trash") }),
        ),
      ),
    );
    return row;
  }

  function startRename(row, category) {
    const input = el("input", { class: "input input--sm", value: category.name, maxlength: "60" });
    const error = el("div", { class: "cat-row__error" });
    const save = async () => {
      const name = input.value.trim();
      if (!name) {
        error.textContent = "Укажите название";
        input.focus();
        return;
      }
      try {
        await api.saveServiceCategory({ id: category.id, name });
        toast("Категория переименована", { type: "success" });
        await load();
      } catch (e) {
        error.textContent = String(e?.message ?? e);
      }
    };
    input.addEventListener("input", () => {
      if (error.textContent) error.textContent = "";
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        save();
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        renderList();
      }
    });
    row.innerHTML = "";
    row.classList.add("cat-row--editing");
    row.append(
      el("div", { class: "cat-row__main" }, input, error),
      el(
        "div",
        { class: "cat-row__actions" },
        el(
          "button",
          { class: "btn btn--ghost btn--icon btn--sm", title: "Сохранить", onclick: save },
          el("span", { html: icon("check") }),
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Отмена",
            onclick: () => renderList(),
          },
          el("span", { html: icon("x") }),
        ),
      ),
    );
    setTimeout(() => {
      input.focus();
      input.select();
    }, 20);
  }

  async function addCategory() {
    const name = newInput.value.trim();
    if (!name) {
      errorEl.textContent = "Укажите название категории";
      newInput.focus();
      return;
    }
    try {
      await api.saveServiceCategory({ id: null, name });
      newInput.value = "";
      newInput.focus();
      toast("Категория добавлена", { type: "success" });
      await load();
    } catch (e) {
      errorEl.textContent = String(e?.message ?? e);
    }
  }

  async function removeCategory(category) {
    const count = category.serviceCount;
    const ok = await confirmDialog({
      title: "Удалить категорию?",
      text:
        count > 0
          ? `«${category.name}» исчезнет из справочника. ${count} ${plural(
              count,
              "услуга останется",
              "услуги останутся",
              "услуг останутся",
            )} в каталоге и ${plural(count, "перейдёт", "перейдут", "перейдут")} в «Без категории».`
          : `«${category.name}» исчезнет из справочника. Услуг в этой категории нет.`,
      confirmText: "Удалить",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.deleteServiceCategory(category.id);
      toast("Категория удалена", { type: "success" });
      await load();
    } catch (e) {
      errorEl.textContent = String(e?.message ?? e);
    }
  }

  load();
  setTimeout(() => newInput.focus(), 40);
}

/* ---------- форма услуги ---------- */

async function openServiceModal({ service = null, onSaved = null } = {}) {
  let categories = [];
  try {
    categories = await api.serviceCategories();
  } catch {
    categories = [];
  }

  const nameInput = el("input", {
    class: "input",
    value: service?.name ?? "",
    placeholder: "Например: фото на документы 3×4, 4 шт.",
  });
  const categorySelect = el("select", { class: "select" });
  const unitInput = el("input", {
    class: "input",
    value: service?.unit ?? "усл.",
    placeholder: "усл., шт., компл.",
  });
  const priceInput = el("input", {
    class: "input input--num",
    value: service ? money(service.priceKop) : "",
    placeholder: "0,00",
    inputmode: "decimal",
  });
  const errorEl = el("div", { class: "modal__error" });

  const fillCategories = (selected = "") => {
    categorySelect.innerHTML = "";
    categorySelect.appendChild(el("option", { value: "" }, "Без категории"));
    for (const category of categories) {
      categorySelect.appendChild(el("option", { value: category.name }, category.name));
    }
    if (selected && !categories.some((c) => c.name.toLowerCase() === selected.toLowerCase())) {
      categorySelect.appendChild(el("option", { value: selected }, selected));
    }
    categorySelect.value = selected;
  };
  fillCategories(service?.category?.trim() ?? "");

  const newCategoryInput = el("input", {
    class: "input input--sm",
    placeholder: "Название новой категории",
    maxlength: "60",
  });
  const newCategoryRow = el(
    "div",
    { class: "cat-inline", hidden: true },
    newCategoryInput,
    el(
      "button",
      { class: "btn btn--primary btn--sm", type: "button", onclick: () => createCategory() },
      "Создать",
    ),
    el(
      "button",
      { class: "btn btn--ghost btn--sm", type: "button", onclick: () => hideNewCategory() },
      "Отмена",
    ),
  );

  function showNewCategory() {
    newCategoryRow.hidden = false;
    newCategoryInput.focus();
  }

  function hideNewCategory() {
    newCategoryRow.hidden = true;
    newCategoryInput.value = "";
  }

  async function createCategory() {
    const name = newCategoryInput.value.trim();
    if (!name) {
      errorEl.textContent = "Укажите название категории";
      newCategoryInput.focus();
      return;
    }
    try {
      await api.saveServiceCategory({ id: null, name });
      const created = await api.serviceCategories();
      categories = created;
      fillCategories(name);
      hideNewCategory();
      errorEl.textContent = "";
      toast("Категория добавлена", { type: "success" });
    } catch (e) {
      errorEl.textContent = String(e?.message ?? e);
    }
  }

  const modal = openModal({
    title: service ? "Изменить услугу" : "Новая услуга",
    sub: "Шаблон для товарного чека",
    body: el(
      "div",
      { class: "stack", style: { gap: "14px" } },
      field("Наименование", nameInput),
      el(
        "div",
        { class: "settings-row" },
        field(
          "Категория",
          el(
            "div",
            { class: "cat-select" },
            categorySelect,
            el(
              "button",
              {
                class: "btn btn--ghost btn--sm cat-select__add",
                type: "button",
                title: "Новая категория",
                onclick: showNewCategory,
              },
              "+ Новая категория",
            ),
          ),
          "Необязательно",
        ),
        field("Единица", unitInput),
      ),
      newCategoryRow,
      field(
        "Цена",
        el(
          "div",
          { class: "input-group" },
          priceInput,
          el("span", { class: "input-suffix" }, "₽"),
        ),
      ),
    ),
    footer: [
      errorEl,
      el("div", { class: "spacer" }),
      el("button", { class: "btn btn--secondary", onclick: () => modal.close() }, "Отмена"),
      el(
        "button",
        {
          class: "btn btn--primary",
          onclick: async () => {
            errorEl.textContent = "";
            const name = nameInput.value.trim();
            if (!name) {
              errorEl.textContent = "Укажите наименование";
              nameInput.focus();
              return;
            }
            const price = parseMoney(priceInput.value);
            if (price === null || price < 0) {
              errorEl.textContent = "Укажите корректную цену";
              priceInput.focus();
              return;
            }
            try {
              await api.saveService({
                id: service?.id ?? null,
                name,
                unit: unitInput.value.trim() || "усл.",
                priceKop: price,
                category: categorySelect.value.trim(),
              });
              modal.close();
              toast(service ? "Услуга обновлена" : "Услуга добавлена", { type: "success", title: rub(price) });
              onSaved?.();
            } catch (e) {
              errorEl.textContent = String(e);
            }
          },
        },
        service ? "Сохранить" : "Добавить",
      ),
    ],
  });

  newCategoryInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      createCategory();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      hideNewCategory();
    }
  });
  newCategoryInput.addEventListener("input", () => {
    if (errorEl.textContent) errorEl.textContent = "";
  });

  setTimeout(() => {
    nameInput.focus();
    nameInput.select();
  }, 30);
}
