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
import { money, payerLabel, plural } from "../format.js";
import { setHeader } from "../shell.js";

let search = "";
let showArchived = false;

export async function render(root) {
  setHeader({
    title: "Клиенты",
    sub: "Плательщики, с которыми вы работаете",
    actions: el(
      "button",
      { class: "btn btn--primary", onclick: () => openClientModal({ onSaved: () => refresh() }) },
      el("span", { html: icon("plus") }),
      "Новый клиент",
    ),
  });

  root.innerHTML = "";
  const page = el("div", { class: "stack", style: { gap: "14px" } });
  const searchInput = el("input", {
    class: "input",
    type: "search",
    placeholder: "Имя, ИНН или заметка",
    value: search,
  });
  const archiveToggle = el(
    "button",
    { class: `chip-toggle${showArchived ? " is-active" : ""}` },
    "Показать архив",
  );
  archiveToggle.addEventListener("click", () => {
    showArchived = !showArchived;
    archiveToggle.classList.toggle("is-active", showArchived);
    refresh();
  });

  const toolbar = el(
    "div",
    { class: "toolbar", style: { marginBottom: "0" } },
    el("div", { class: "toolbar__search" }, el("span", { html: icon("search") }), searchInput),
    archiveToggle,
    el("div", { class: "toolbar__spacer" }),
    el("div", { class: "small muted", id: "clients-count" }),
  );

  const card = el("div", { class: "card" });
  page.append(toolbar, card);
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
    let clients = [];
    try {
      clients = await api.clients(showArchived);
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

    const q = search.toLowerCase();
    const visible = q
      ? clients.filter((c) =>
          `${c.name} ${c.inn} ${c.note}`.toLowerCase().includes(q),
        )
      : clients;
    const countEl = document.getElementById("clients-count");
    if (countEl) {
      countEl.textContent = `${visible.length} из ${clients.length}`;
    }

    const active = visible.filter((c) => !c.archived).length;
    card.appendChild(
      el(
        "div",
        { class: "card__head" },
        el("h3", { class: "card__title" }, "Справочник клиентов"),
        el(
          "span",
          { class: "card__sub" },
          `${active} ${plural(active, "активный", "активных", "активных")}`,
        ),
      ),
    );

    if (visible.length === 0) {
      card.appendChild(
        emptyState({
          iconName: "users",
          title: q ? "Никого не нашли" : "Клиентов пока нет",
          text: q
            ? "Попробуйте другой запрос."
            : "Добавьте клиентов, чтобы быстрее заполнять доходы и товарные чеки.",
          action: q
            ? null
            : el(
                "button",
                { class: "btn btn--primary btn--sm", onclick: () => openClientModal({ onSaved: () => refresh() }) },
                "Добавить клиента",
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
          el("th", {}, "Клиент"),
          el("th", { style: { width: "140px" } }, "ИНН"),
          el("th", { style: { width: "110px" } }, "Тип"),
          el("th", { class: "col-num", style: { width: "100px" } }, "Операций"),
          el("th", { class: "col-num", style: { width: "150px" } }, "Сумма"),
          el("th", { class: "col-actions", style: { width: "118px" } }),
        ),
      ),
    );
    const tbody = el("tbody");
    for (const client of visible) {
      tbody.appendChild(clientRow(client));
    }
    table.appendChild(tbody);
    card.appendChild(el("div", { class: "table-wrap" }, table));
  }

  function clientRow(client) {
    return el(
      "tr",
      {},
      el(
        "td",
        {},
        el(
          "div",
          { class: "cell-title" },
          el(
            "span",
            { class: "cell-strong" },
            client.name,
            client.archived
              ? el("span", { class: "badge badge--outline", style: { marginLeft: "6px" } }, "архив")
              : null,
          ),
          client.note ? el("small", {}, client.note) : null,
        ),
      ),
      el("td", { class: "cell-muted" }, client.inn || "—"),
      el(
        "td",
        {},
        el(
          "span",
          { class: client.kind === "organization" ? "badge badge--accent" : "badge" },
          payerLabel(client.kind),
        ),
      ),
      el("td", { class: "col-num cell-muted" }, String(client.entriesCount)),
      el(
        "td",
        { class: "col-num cell-strong" },
        client.totalKop > 0 ? `${money(client.totalKop)} ₽` : "—",
      ),
      el(
        "td",
        { class: "col-actions" },
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Изменить",
            onclick: () => openClientModal({ client, onSaved: () => refresh() }),
          },
          el("span", { html: icon("edit") }),
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: client.archived ? "Вернуть из архива" : "В архив",
            onclick: async () => {
              try {
                await api.setClientArchived(client.id, !client.archived);
                toast(client.archived ? "Клиент возвращён" : "Клиент в архиве", { type: "success" });
                refresh();
              } catch (e) {
                toastError(e);
              }
            },
          },
          el("span", { html: icon(client.archived ? "refresh" : "folder") }),
        ),
        el(
          "button",
          {
            class: "btn btn--ghost btn--icon btn--sm",
            title: "Удалить",
            onclick: async () => {
              const ok = await confirmDialog({
                title: "Удалить клиента?",
                text: client.entriesCount
                  ? "У клиента есть операции — удаление невозможно. Отправьте его в архив."
                  : `«${client.name}» будет удалён из справочника.`,
                confirmText: "Удалить",
                danger: true,
              });
              if (!ok) return;
              try {
                await api.deleteClient(client.id);
                toast("Клиент удалён", { type: "success" });
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

  await refresh();
}

function openClientModal({ client = null, onSaved = null } = {}) {
  let kind = client?.kind ?? "individual";

  const nameInput = el("input", {
    class: "input",
    value: client?.name ?? "",
    placeholder: "ФИО или название организации",
    autofocus: true,
  });
  const innInput = el("input", {
    class: "input",
    value: client?.inn ?? "",
    placeholder: "10 или 12 цифр",
    inputmode: "numeric",
  });
  const noteInput = el("textarea", {
    class: "textarea",
    rows: "2",
    placeholder: "Например: постоянный клиент, фото на документы",
  });
  noteInput.value = client?.note ?? "";

  const kindWrap = el("div", { class: "segmented" });
  const buttons = [];
  for (const [value, label] of [
    ["individual", "Физлицо"],
    ["organization", "Юрлицо"],
  ]) {
    const btn = el("button", { type: "button", class: value === kind ? "is-active" : "" }, label);
    btn.addEventListener("click", () => {
      kind = value;
      buttons.forEach((b) => b.classList.remove("is-active"));
      btn.classList.add("is-active");
    });
    buttons.push(btn);
    kindWrap.appendChild(btn);
  }

  const errorEl = el("div", { class: "modal__error" });

  const modal = openModal({
    title: client ? "Изменить клиента" : "Новый клиент",
    sub: "Тип клиента определяет ставку налога в доходах",
    body: el(
      "div",
      { class: "stack", style: { gap: "14px" } },
      field("Имя или название", nameInput),
      el(
        "div",
        { class: "settings-row" },
        field("ИНН", innInput, "Необязательно"),
        field("Тип", kindWrap),
      ),
      field("Заметка", noteInput),
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
              errorEl.textContent = "Укажите имя или название";
              nameInput.focus();
              return;
            }
            const inn = innInput.value.trim();
            if (inn && !/^\d{10}(\d{2})?$/.test(inn)) {
              errorEl.textContent = "ИНН — 10 или 12 цифр";
              innInput.focus();
              return;
            }
            try {
              await api.saveClient({
                id: client?.id ?? null,
                name,
                inn,
                kind,
                note: noteInput.value,
              });
              modal.close();
              toast(client ? "Клиент обновлён" : "Клиент добавлен", { type: "success" });
              onSaved?.();
            } catch (e) {
              errorEl.textContent = String(e);
            }
          },
        },
        client ? "Сохранить" : "Добавить",
      ),
    ],
  });

  setTimeout(() => nameInput.focus(), 30);
  modal.bodyEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey && e.target.tagName !== "TEXTAREA") {
      e.preventDefault();
      modal.root.querySelector(".btn--primary")?.click();
    }
  });
}
