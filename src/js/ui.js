const ICONS = {
  grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  wallet: '<path d="M3 8.5A2.5 2.5 0 0 1 5.5 6H19a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5.5A2.5 2.5 0 0 1 3 16.5z"/><path d="M3 9h18"/><circle cx="16.5" cy="14" r="1.2"/>',
  receipt: '<path d="M6 3h12v18l-2.2-1.6L13.6 21l-2.3-1.6L9 21l-1.4-1.1L6 21z"/><path d="M9.5 8h5M9.5 12h5"/>',
  users: '<circle cx="9" cy="8" r="3.2"/><path d="M3.5 20c0-3 2.5-5.2 5.5-5.2s5.5 2.2 5.5 5.2"/><path d="M15.5 5.4a3 3 0 0 1 0 5.2M17 14.9c2.1.5 3.5 2.3 3.5 4.6"/>',
  list: '<path d="M8 6h12M8 12h12M8 18h12"/><circle cx="4" cy="6" r="1"/><circle cx="4" cy="12" r="1"/><circle cx="4" cy="18" r="1"/>',
  chart: '<path d="M4 20V10M10 20V4M16 20v-7M22 20H2"/>',
  settings: '<circle cx="12" cy="12" r="3.2"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
  edit: '<path d="M4 20h4l10-10a2.8 2.8 0 0 0-4-4L4 16z"/><path d="m13.5 6.5 4 4"/>',
  tag: '<path d="M20.6 13.6 12.4 21.8a2 2 0 0 1-2.8 0L3 15.2V4h11.2l6.4 6.4a2 2 0 0 1 0 3.2z"/><circle cx="7.5" cy="7.5" r="1.3"/>',
  file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
  check: '<path d="m5 12 5 5 9-10"/>',
  checkCircle: '<circle cx="12" cy="12" r="9"/><path d="m8.5 12.2 2.4 2.4 4.6-4.9"/>',
  alert: '<path d="M12 4 3 20h18z"/><path d="M12 10v4.5M12 17.4v.2"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5.5M12 7.6v.2"/>',
  x: '<path d="M6 6l12 12M18 6 6 18"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M19.1 4.9l-1.4 1.4M6.3 17.7l-1.4 1.4"/>',
  moon: '<path d="M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z"/>',
  left: '<path d="m14 6-6 6 6 6"/>',
  right: '<path d="m10 6 6 6-6 6"/>',
  up: '<path d="m6 14 6-6 6 6"/>',
  down: '<path d="m6 10 6 6 6-6"/>',
  upload: '<path d="M12 16V4M7 9l5-5 5 5"/><path d="M4 16v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3"/>',
  print: '<path d="M7 8V3h10v5"/><rect x="4" y="8" width="16" height="8" rx="2"/><path d="M7 16h10v5H7z"/>',
  copy: '<rect x="9" y="9" width="11" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h8"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h5l2 2.5h7a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  refresh: '<path d="M20 12a8 8 0 1 1-2.4-5.7"/><path d="M20 4v5h-5"/>',
  download: '<path d="M12 4v12M7 11l5 5 5-5"/><path d="M4 20h16"/>',
  qr: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><path d="M14 14h3v3h-3zM20 14v.1M14 20v.1M20 20v.1"/>',
  sparkles: '<path d="M12 3l1.6 4.4L18 9l-4.4 1.6L12 15l-1.6-4.4L6 9l4.4-1.6z"/><path d="M18 15l.8 2.2L21 18l-2.2.8L18 21l-.8-2.2L15 18l2.2-.8z"/>',
};

export function icon(name, cls = "") {
  const body = ICONS[name] ?? ICONS.info;
  return `<svg class="${cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;
}

export function iconEl(name, cls = "") {
  const span = document.createElement("span");
  span.style.display = "contents";
  span.innerHTML = icon(name, cls);
  return span.firstElementChild;
}

function appendChildren(node, children) {
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    if (Array.isArray(child)) {
      appendChildren(node, child);
    } else if (child instanceof Node) {
      node.appendChild(child);
    } else {
      node.appendChild(document.createTextNode(String(child)));
    }
  }
}

export function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value === null || value === undefined || value === false) continue;
    if (key === "class") node.className = value;
    else if (key === "html") node.innerHTML = value;
    else if (key === "text") node.textContent = value;
    else if (key === "dataset") Object.assign(node.dataset, value);
    else if (key === "style" && typeof value === "object") Object.assign(node.style, value);
    else if (key.startsWith("on") && typeof value === "function") {
      node.addEventListener(key.slice(2).toLowerCase(), value);
    } else if (value === true) {
      node.setAttribute(key, "");
    } else {
      node.setAttribute(key, String(value));
    }
  }
  appendChildren(node, children);
  return node;
}

export function debounce(fn, ms = 300) {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

/* ---------- тосты ---------- */

let toastHost = null;

function host() {
  if (!toastHost) {
    toastHost = el("div", { class: "toasts" });
    document.body.appendChild(toastHost);
  }
  return toastHost;
}

export function toast(message, { type = "info", title = "", actions = [], timeout = 4200 } = {}) {
  const node = el(
    "div",
    { class: `toast toast--${type}` },
    el(
      "div",
      { class: "toast__body" },
      title ? el("div", { class: "toast__title" }, title) : null,
      el("div", { class: "toast__text" }, message),
      actions.length
        ? el(
            "div",
            { class: "toast__actions" },
            ...actions.map((a) =>
              el(
                "button",
                {
                  class: "btn btn--sm btn--secondary",
                  onclick: () => {
                    a.onClick?.();
                    remove();
                  },
                },
                a.label,
              ),
            ),
          )
        : null,
    ),
  );
  const remove = () => {
    node.style.opacity = "0";
    node.style.transform = "translateY(6px)";
    node.style.transition = "opacity .16s, transform .16s";
    setTimeout(() => node.remove(), 170);
  };
  host().appendChild(node);
  if (timeout) setTimeout(remove, timeout);
  return remove;
}

export const toastError = (e) =>
  toast(String(e?.message ?? e ?? "Неизвестная ошибка"), { type: "error", title: "Не получилось", timeout: 7000 });

/* ---------- модальные окна ---------- */

const modalStack = [];

export function openModal({
  title = "",
  sub = "",
  size = "",
  body = null,
  footer = null,
  onClose = null,
  closeOnBackdrop = true,
}) {
  const overlay = el("div", { class: "overlay" });
  const dialog = el("div", { class: `modal${size ? ` modal--${size}` : ""}` });
  const head = el(
    "div",
    { class: "modal__head" },
    el(
      "div",
      {},
      el("h3", { class: "modal__title" }, title),
      sub ? el("div", { class: "modal__sub" }, sub) : null,
    ),
    el(
      "button",
      { class: "btn btn--ghost btn--icon modal__close", title: "Закрыть", onclick: () => close() },
      iconEl("x"),
    ),
  );
  const bodyEl = el("div", { class: "modal__body" });
  if (body) bodyEl.appendChild(body);
  const footEl = footer ? el("div", { class: "modal__foot" }, footer) : null;

  dialog.append(head, bodyEl);
  if (footEl) dialog.appendChild(footEl);
  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  const entry = { close };
  modalStack.push(entry);

  function close() {
    const idx = modalStack.indexOf(entry);
    if (idx >= 0) modalStack.splice(idx, 1);
    overlay.remove();
    onClose?.();
  }

  if (closeOnBackdrop) {
    overlay.addEventListener("mousedown", (e) => {
      if (e.target === overlay) close();
    });
  }

  return { root: dialog, bodyEl, footEl, close, overlay };
}

export function confirmDialog({
  title = "Подтвердите действие",
  text = "",
  confirmText = "Продолжить",
  cancelText = "Отмена",
  danger = false,
} = {}) {
  return new Promise((resolve) => {
    let decided = false;
    const modal = openModal({
      title,
      body: el("div", { class: "text-2", style: { fontSize: "13px", lineHeight: "1.5" } }, text),
      onClose: () => {
        if (!decided) resolve(false);
      },
    });
    const cancel = el(
      "button",
      {
        class: "btn btn--secondary",
        onclick: () => {
          decided = true;
          resolve(false);
          modal.close();
        },
      },
      cancelText,
    );
    const ok = el(
      "button",
      {
        class: `btn ${danger ? "btn--primary" : "btn--primary"}`,
        style: danger ? { background: "var(--danger)", borderColor: "var(--danger)" } : {},
        onclick: () => {
          decided = true;
          resolve(true);
          modal.close();
        },
      },
      confirmText,
    );
    modal.root.appendChild(el("div", { class: "modal__foot" }, el("div", { class: "spacer" }), cancel, ok));
  });
}

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && modalStack.length) {
    e.preventDefault();
    modalStack[modalStack.length - 1].close();
  }
});

/* ---------- прочее ---------- */

export function emptyState({ iconName = "file", title, text, action = null }) {
  return el(
    "div",
    { class: "empty" },
    el("div", { class: "empty__icon", html: icon(iconName) }),
    el("div", { class: "empty__title" }, title),
    text ? el("div", { class: "empty__text" }, text) : null,
    action,
  );
}

export function field(label, control, hint = "") {
  return el(
    "label",
    { class: "field" },
    label ? el("span", { class: "field__label" }, label) : null,
    control,
    hint ? el("span", { class: "field__hint" }, hint) : null,
  );
}

export function input(props = {}) {
  const { type = "text", ...rest } = props;
  return el("input", { class: "input", type, ...rest });
}

export function select(props = {}, options = []) {
  const node = el("select", { class: "select", ...props });
  for (const opt of options) {
    node.appendChild(el("option", { value: opt.value, selected: opt.selected }, opt.label));
  }
  return node;
}

export function notice(kind, text, iconName = "info") {
  return el("div", { class: `notice notice--${kind}` }, el("div", { html: icon(iconName) }), el("div", {}, text));
}
