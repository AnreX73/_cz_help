import { api } from "../api.js";
import {
  el,
  icon,
  field,
  openModal,
  confirmDialog,
  toast,
  toastError,
} from "../ui.js";
import { state, updateSidebar } from "../shell.js";

const DEFAULT_STATUS = "Плательщик налога на профессиональный доход";

export async function openSellersModal() {
  const modal = openModal({
    title: "Самозанятые",
    sub: "Доходы, налог, вычет и детализация заказов считаются по активному профилю",
    size: "sm",
  });

  const list = el("div", { class: "seller-list" });
  const body = el("div", { class: "stack", style: { gap: "12px" } }, list);
  modal.bodyEl.appendChild(body);

  const addBtn = el(
    "button",
    {
      class: "btn btn--primary",
      onclick: () => openSellerForm({ onSaved: () => renderList() }),
    },
    el("span", { html: icon("plus") }),
    "Добавить",
  );
  const closeBtn = el("button", { class: "btn btn--secondary", onclick: () => modal.close() }, "Закрыть");
  modal.root.appendChild(el("div", { class: "modal__foot" }, el("div", { class: "spacer" }), closeBtn, addBtn));

  async function renderList() {
    try {
      state.sellers = await api.sellers(true);
      state.seller = await api.activeSeller();
      updateSidebar();
    } catch (e) {
      toastError(e);
      return;
    }
    list.innerHTML = "";
    for (const seller of state.sellers) {
      const active = state.seller?.id === seller.id;
      const row = el(
        "div",
        {
          class: `seller-row${active ? " is-active" : ""}${seller.archived ? " is-archived" : ""}`,
        },
        el(
          "button",
          {
            class: "seller-row__main",
            type: "button",
            title: active ? "Активный профиль" : "Сделать активным",
            onclick: async () => {
              if (active || seller.archived) return;
              try {
                await switchToSeller(seller.id);
                renderList();
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
          el(
            "span",
            { class: "seller-row__meta" },
            seller.inn ? `ИНН ${seller.inn}` : "ИНН не заполнен",
          ),
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
              onclick: () => openSellerForm({ seller, onSaved: () => renderList() }),
            },
            el("span", { html: icon("edit") }),
          ),
        ),
      );
      list.appendChild(row);
    }
    if (state.sellers.length === 0) {
      list.appendChild(el("div", { class: "small muted" }, "Пока нет ни одного профиля."));
    }
  }

  await renderList();
}

export async function switchToSeller(id) {
  const seller = await api.switchSeller(id);
  state.seller = seller;
  state.sellers = await api.sellers(true);
  updateSidebar();
  toast(`Профиль: ${seller.name || "без имени"}`, { type: "success", timeout: 2400 });
  window.dispatchEvent(new CustomEvent("cz:seller-changed"));
  return seller;
}

export function openSellerForm({ seller = null, prefillInn = "", onSaved = null } = {}) {
  const editing = Boolean(seller);
  const nameInput = el("input", {
    class: "input",
    value: seller?.name ?? "",
    placeholder: "Иванов Иван Иванович",
    autofocus: true,
  });
  const innInput = el("input", {
    class: "input input--num",
    value: seller?.inn || prefillInn,
    placeholder: "12 цифр",
    inputmode: "numeric",
    maxlength: "12",
  });
  const cityInput = el("input", { class: "input", value: seller?.city ?? "", placeholder: "Город" });
  const contactInput = el("input", {
    class: "input",
    value: seller?.contact ?? "",
    placeholder: "Телефон, e-mail или сайт",
  });
  const statusInput = el("input", {
    class: "input",
    value: seller?.status ?? DEFAULT_STATUS,
    placeholder: DEFAULT_STATUS,
  });
  const errorEl = el("div", { class: "field__hint", style: { color: "var(--danger)" } });

  const body = el(
    "div",
    { class: "stack", style: { gap: "12px" } },
    field("Имя или наименование", nameInput),
    field("ИНН", innInput, "12 цифр, как в приложении «Мой налог»"),
    el(
      "div",
      { class: "settings-row" },
      field("Город", cityInput),
      field("Статус", statusInput),
    ),
    field("Контакт", contactInput, "Телефон, e-mail или сайт — по желанию"),
    errorEl,
  );

  const modal = openModal({
    title: editing ? "Изменить самозанятого" : "Новый самозанятый",
    sub: editing ? "Документы сохранят прежние данные" : "Профиль для расчёта налога и печати детализации заказов",
    body,
  });

  const saveBtn = el(
    "button",
    {
      class: "btn btn--primary",
      onclick: async () => {
        errorEl.textContent = "";
        const name = nameInput.value.trim();
        const inn = innInput.value.trim();
        if (!name) {
          errorEl.textContent = "Укажите имя самозанятого";
          nameInput.focus();
          return;
        }
        if (inn && !/^\d{12}$/.test(inn)) {
          errorEl.textContent = "ИНН — ровно 12 цифр";
          innInput.focus();
          return;
        }
        saveBtn.disabled = true;
        try {
          const saved = await api.saveSeller({
            id: seller?.id ?? null,
            name,
            inn,
            city: cityInput.value.trim(),
            contact: contactInput.value.trim(),
            status: statusInput.value.trim(),
          });
          state.sellers = await api.sellers(true);
          state.seller = await api.activeSeller();
          updateSidebar();
          modal.close();
          toast(editing ? "Профиль обновлён" : "Самозанятый добавлен", { type: "success", timeout: 2600 });
          onSaved?.(saved);
          if (!editing) window.dispatchEvent(new CustomEvent("cz:seller-changed"));
        } catch (e) {
          errorEl.textContent = String(e?.message ?? e);
        } finally {
          saveBtn.disabled = false;
        }
      },
    },
    el("span", { html: icon("check") }),
    editing ? "Сохранить" : "Добавить",
  );

  const footer = [el("div", { class: "spacer" })];
  if (editing) {
    footer.push(
      el(
        "button",
        {
          class: "btn btn--secondary",
          onclick: async () => {
            try {
              await api.setSellerArchived(seller.id, !seller.archived);
              state.sellers = await api.sellers(true);
              modal.close();
              toast(seller.archived ? "Профиль возвращён" : "Профиль в архиве", { type: "success" });
              onSaved?.();
            } catch (e) {
              toastError(e);
            }
          },
        },
        seller.archived ? "Вернуть из архива" : "В архив",
      ),
    );
    if (state.seller?.id !== seller.id) {
      footer.push(
        el(
          "button",
          {
            class: "btn btn--secondary",
            style: { color: "var(--danger)" },
            onclick: async () => {
              const ok = await confirmDialog({
                title: "Удалить самозанятого?",
                text: `Профиль «${seller.name}» будет удалён. Удаление возможно, только если у него нет доходов и детализаций заказов.`,
                confirmText: "Удалить",
                danger: true,
              });
              if (!ok) return;
              try {
                await api.deleteSeller(seller.id);
                state.sellers = await api.sellers(true);
                modal.close();
                toast("Профиль удалён", { type: "success" });
                onSaved?.();
              } catch (e) {
                toastError(e);
              }
            },
          },
          "Удалить",
        ),
      );
    }
  }
  footer.push(
    el("button", { class: "btn btn--secondary", onclick: () => modal.close() }, "Отмена"),
    saveBtn,
  );
  modal.root.appendChild(el("div", { class: "modal__foot" }, ...footer));
}
