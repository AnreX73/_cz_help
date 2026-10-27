import { api } from "./api.js";
import { icon, el, toast } from "./ui.js";

export const state = {
  settings: null,
  seller: null,
  sellers: [],
  paths: null,
  route: "dashboard",
};

const NAV = [
  {
    group: "Работа",
    items: [
      { route: "dashboard", label: "Обзор", icon: "grid" },
      { route: "income", label: "Доходы", icon: "wallet" },
      { route: "documents", label: "Товарные чеки", icon: "receipt" },
    ],
  },
  {
    group: "Справочники",
    items: [
      { route: "clients", label: "Клиенты", icon: "users" },
      { route: "services", label: "Услуги", icon: "list" },
    ],
  },
  {
    group: "Аналитика",
    items: [{ route: "reports", label: "Отчёты", icon: "chart" }],
  },
];

const mq = window.matchMedia("(prefers-color-scheme: dark)");

function resolvedTheme(theme) {
  if (theme === "system") return mq.matches ? "dark" : "light";
  return theme === "dark" ? "dark" : "light";
}

export function applyTheme(theme) {
  const value = theme || "light";
  const resolved = resolvedTheme(value);
  document.documentElement.dataset.theme = resolved;
  localStorage.setItem("cz_theme", value);
  import("@tauri-apps/api/window")
    .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(resolved))
    .catch(() => {});
}

mq.addEventListener("change", () => {
  if ((state.settings?.theme ?? localStorage.getItem("cz_theme")) === "system") {
    applyTheme("system");
  }
});

export function mountShell() {
  const sidebar = document.getElementById("sidebar");
  sidebar.innerHTML = "";

  sidebar.appendChild(
    el(
      "div",
      { class: "brand" },
      el("div", { class: "brand__mark", html: icon("receipt") }),
      el(
        "div",
        { class: "brand__text" },
        el("span", { class: "brand__title" }, "Помощник самозанятого"),
        el("span", { class: "brand__sub" }, "НПД · товарные чеки"),
      ),
    ),
  );

  for (const group of NAV) {
    sidebar.appendChild(el("div", { class: "nav-group" }, group.group));
    for (const item of group.items) {
      sidebar.appendChild(
        el(
          "a",
          { class: "nav-item", href: `#/${item.route}`, dataset: { route: item.route } },
          el("span", { html: icon(item.icon) }),
          el("span", {}, item.label),
        ),
      );
    }
  }

  sidebar.appendChild(el("div", { class: "sidebar__spacer" }));

  const sellerBtn = el(
    "button",
    { class: "seller-switch", type: "button", title: "Сменить самозанятого" },
    el("span", { class: "seller-switch__icon", html: icon("users") }),
    el(
      "span",
      { class: "seller-switch__text" },
      el("span", { class: "seller-switch__name" }, "Самозанятый"),
      el("span", { class: "seller-switch__meta" }, "не выбран"),
    ),
    el("span", { class: "seller-switch__chevron", html: icon("down") }),
  );
  sellerBtn.addEventListener("click", () => {
    import("./views/sellers.js").then((mod) => mod.openSellersModal());
  });

  const themeBtn = el(
    "button",
    { class: "btn btn--ghost", style: { width: "100%", justifyContent: "flex-start" } },
    el("span", { html: icon("moon") }),
    el("span", { class: "theme-label" }, "Тёмная тема"),
  );

  const footer = el(
    "div",
    { class: "sidebar__footer" },
    sellerBtn,
    el(
      "a",
      { class: "nav-item", href: "#/settings", dataset: { route: "settings" } },
      el("span", { html: icon("settings") }),
      el("span", {}, "Настройки"),
    ),
    themeBtn,
  );
  sidebar.appendChild(footer);

  themeBtn.addEventListener("click", async () => {
    if (!state.settings) return;
    const next = state.settings.theme === "dark" ? "light" : "dark";
    state.settings.theme = next;
    applyTheme(next);
    try {
      state.settings = await api.saveSettings(state.settings);
      updateSidebar();
    } catch (e) {
      toast(String(e), { type: "error" });
    }
  });

  updateSidebar();
}

export function updateSidebar() {
  const sidebar = document.getElementById("sidebar");
  if (!sidebar) return;
  const s = state.settings;
  const seller = state.seller;
  const switchBtn = sidebar.querySelector(".seller-switch");
  if (switchBtn) {
    const nameEl = switchBtn.querySelector(".seller-switch__name");
    const metaEl = switchBtn.querySelector(".seller-switch__meta");
    if (seller && (seller.name || seller.inn)) {
      nameEl.textContent = seller.name || "Без имени";
      metaEl.textContent = seller.inn ? `ИНН ${seller.inn}` : "ИНН не заполнен";
      switchBtn.classList.remove("is-empty");
      switchBtn.title = `${seller.name || "Без имени"}${seller.inn ? ` · ИНН ${seller.inn}` : ""} — сменить`;
    } else {
      nameEl.textContent = "Профиль не заполнен";
      metaEl.textContent = "Добавьте самозанятого";
      switchBtn.classList.add("is-empty");
      switchBtn.title = "Добавить самозанятого";
    }
  }
  const themeBtn = sidebar.querySelector(".sidebar__footer .btn");
  if (themeBtn) {
    const dark = resolvedTheme(s?.theme ?? "light") === "dark";
    themeBtn.innerHTML = "";
    themeBtn.append(
      el("span", { html: icon(dark ? "sun" : "moon") }),
      el("span", { class: "theme-label" }, dark ? "Светлая тема" : "Тёмная тема"),
    );
  }
}

export function setSidebarActive(route) {
  state.route = route;
  for (const link of document.querySelectorAll(".nav-item[data-route]")) {
    link.classList.toggle("is-active", link.dataset.route === route);
  }
}

export function setHeader({ title = "", sub = "", actions = null } = {}) {
  const topbar = document.getElementById("topbar");
  topbar.innerHTML = "";
  topbar.append(
    el(
      "div",
      {},
      el("h1", { class: "topbar__title" }, title),
      sub ? el("div", { class: "topbar__sub" }, sub) : null,
    ),
    el("div", { class: "topbar__actions" }, actions ?? ""),
  );
}

export function navigate(route) {
  if (location.hash === `#/${route}`) return;
  location.hash = `#/${route}`;
}

export async function reloadSettings() {
  state.settings = await api.settings();
  applyTheme(state.settings.theme);
  updateSidebar();
  return state.settings;
}

export async function refreshSellers() {
  state.sellers = await api.sellers(true);
  state.seller = await api.activeSeller();
  updateSidebar();
  return state.seller;
}
