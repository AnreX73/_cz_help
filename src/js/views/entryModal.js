import { api } from "../api.js";
import { openModal, el, field, input, select, toast, toastError, icon, confirmDialog } from "../ui.js";
import { money, parseMoney, todayIso, entriesWord } from "../format.js";

export async function openEntryModal({ entry = null, onSaved = null } = {}) {
  let clients = [];
  try {
    clients = await api.clients(true);
    // В новой операции архивные клиенты не нужны, но у редактируемой
    // операции клиент мог быть отправлен в архив — его нельзя потерять.
    if (!entry?.clientId) clients = clients.filter((c) => !c.archived);
  } catch (e) {
    toastError(e);
  }

  let payerType = entry?.payerType ?? "individual";
  let clientId = entry?.clientId ?? null;

  const dateInput = input({ type: "date", value: entry?.date ?? todayIso() });
  const amountInput = input({
    class: "input input--num",
    inputmode: "decimal",
    value: entry ? money(entry.amountKop) : "",
    placeholder: "0,00",
    autofocus: true,
  });
  const descInput = input({
    value: entry?.description ?? "",
    placeholder: "Например: фото на документы",
  });
  const receiptInput = input({ value: entry?.receiptNo ?? "", placeholder: "Номер чека НПД" });
  const issuedInput = el("input", { type: "checkbox", checked: entry?.receiptIssued ?? false });

  const payerButtons = [];
  const payerWrap = el("div", { class: "segmented" });
  for (const [value, label] of [
    ["individual", "Физлицо"],
    ["organization", "Юрлицо"],
  ]) {
    const btn = el("button", { type: "button", class: value === payerType ? "is-active" : "" }, label);
    btn.addEventListener("click", () => {
      payerType = value;
      payerButtons.forEach((b) => b.classList.remove("is-active"));
      btn.classList.add("is-active");
    });
    payerButtons.push(btn);
    payerWrap.appendChild(btn);
  }

  const clientSelect = select({ class: "select" }, [
    { value: "", label: "Без клиента" },
    ...clients.map((c) => ({
      value: String(c.id),
      label: `${c.name}${c.archived ? " (архив)" : ""}${c.inn ? ` · ${c.inn}` : ""}`,
      selected: String(entry?.clientId ?? "") === String(c.id),
    })),
  ]);
  clientSelect.addEventListener("change", () => {
    const id = clientSelect.value ? Number(clientSelect.value) : null;
    clientId = id;
    const client = clients.find((c) => c.id === id);
    if (client) {
      payerType = client.kind;
      payerButtons.forEach((b) => b.classList.remove("is-active"));
      const idx = payerType === "organization" ? 1 : 0;
      payerButtons[idx].classList.add("is-active");
    }
  });

  const errorEl = el("div", { class: "modal__error" });

  const body = el(
    "div",
    { class: "stack", style: { gap: "14px" } },
    el(
      "div",
      { class: "settings-row" },
      field("Дата", dateInput),
      field(
        "Сумма",
        el("div", { class: "input-group" }, amountInput, el("span", { class: "input-suffix" }, "₽")),
      ),
    ),
    el(
      "div",
      { class: "settings-row" },
      field("Тип плательщика", payerWrap, "От него зависит ставка: 4 % или 6 %"),
      field("Клиент", clientSelect, "Необязательно, но пригодится в отчётах"),
    ),
    field("Описание", descInput),
    el(
      "div",
      { class: "settings-row" },
      field("Номер чека", receiptInput, "Заполнится, когда выбьете чек в «Мой налог»"),
      el(
        "label",
        { class: "field", style: { flexDirection: "row", alignItems: "center", gap: "8px", paddingTop: "20px" } },
        issuedInput,
        el("span", { class: "field__label" }, "Чек выдан"),
      ),
    ),
  );

  const modal = openModal({
    title: entry ? "Изменить доход" : "Новый доход",
    sub: entry ? "Правка пересчитает налог за период" : "Налог и лимит пересчитаются автоматически",
    body,
    footer: [
      errorEl,
      entry
        ? el(
            "button",
            {
              class: "btn btn--danger",
              onclick: async () => {
                const ok = await confirmDialog({
                  title: "Удалить операцию?",
                  text: "Это действие нельзя отменить. Налог будет пересчитан.",
                  confirmText: "Удалить",
                  danger: true,
                });
                if (!ok) return;
                try {
                  await api.deleteEntry(entry.id);
                  modal.close();
                  onSaved?.();
                  toast("Операция удалена", { type: "success" });
                } catch (e) {
                  toastError(e);
                }
              },
            },
            el("span", { html: icon("trash") }),
            "Удалить",
          )
        : el("div", {}),
      el("div", { class: "spacer" }),
      el("button", { class: "btn btn--secondary", onclick: () => modal.close() }, "Отмена"),
      el(
        "button",
        {
          class: "btn btn--primary",
          onclick: async () => {
            errorEl.textContent = "";
            const kop = parseMoney(amountInput.value);
            if (!dateInput.value) {
              errorEl.textContent = "Укажите дату";
              return;
            }
            if (kop === null || kop <= 0) {
              errorEl.textContent = "Укажите сумму больше нуля";
              amountInput.classList.add("input--invalid");
              return;
            }
            amountInput.classList.remove("input--invalid");
            try {
              await api.saveEntry({
                id: entry?.id ?? null,
                date: dateInput.value,
                amountKop: kop,
                payerType,
                clientId,
                description: descInput.value,
                receiptNo: receiptInput.value,
                receiptIssued: issuedInput.checked,
              });
              modal.close();
              onSaved?.();
              toast(entry ? "Операция обновлена" : "Доход добавлен", {
                type: "success",
                title: entry ? undefined : `${money(kop)} ₽`,
              });
            } catch (e) {
              errorEl.textContent = String(e);
            }
          },
        },
        entry ? "Сохранить" : "Добавить",
      ),
    ],
  });

  setTimeout(() => amountInput.focus(), 30);
  const onEnter = (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      modal.root.querySelector(".btn--primary")?.click();
    }
  };
  body.addEventListener("keydown", onEnter);

  return modal;
}

export async function quickAddEntry() {
  await openEntryModal({
    onSaved: () => window.dispatchEvent(new CustomEvent("cz:data-changed")),
  });
}
