import { api } from "./api.js";
import { applyTheme, mountShell, setHeader, setSidebarActive, state } from "./shell.js";
import { el, icon, toastError } from "./ui.js";
import { openEntryModal } from "./views/entryModal.js";
import { handleDroppedPath, openWizard } from "./views/documentWizard.js";

const VIEWS = {
  dashboard: () => import("./views/dashboard.js"),
  income: () => import("./views/income.js"),
  clients: () => import("./views/clients.js"),
  services: () => import("./views/services.js"),
  documents: () => import("./views/documents.js"),
  reports: () => import("./views/reports.js"),
  settings: () => import("./views/settings.js"),
};

let renderToken = 0;
let dropOverlay = null;

function currentRoute() {
  const raw = location.hash.replace(/^#\/?/, "").split("?")[0];
  return VIEWS[raw] ? raw : "dashboard";
}

async function renderRoute() {
  const route = currentRoute();
  setSidebarActive(route);
  const view = document.getElementById("view");
  const token = ++renderToken;

  view.className = "view";
  view.innerHTML = "";
  view.appendChild(
    el(
      "div",
      { class: "empty", style: { paddingTop: "80px" } },
      el("div", { class: "empty__icon", html: icon("refresh", "spin") }),
      el("div", { class: "empty__text" }, "Загрузка…"),
    ),
  );

  try {
    const mod = await VIEWS[route]();
    if (token !== renderToken) return;
    await mod.render(view);
  } catch (e) {
    if (token !== renderToken) return;
    view.innerHTML = "";
    setHeader({ title: "Ошибка", sub: "Не удалось открыть раздел" });
    view.appendChild(
      el(
        "div",
        { class: "notice notice--danger" },
        el("div", { html: icon("alert") }),
        el("div", {}, String(e?.message ?? e)),
      ),
    );
    toastError(e);
  }
}

/* ---------- drag & drop ---------- */

function ensureDropOverlay() {
  if (dropOverlay) return dropOverlay;
  dropOverlay = el(
    "div",
    {
      style: {
        position: "fixed",
        inset: "0",
        zIndex: "80",
        display: "none",
        placeItems: "center",
        background: "color-mix(in srgb, var(--bg) 72%, transparent)",
        backdropFilter: "blur(3px)",
        pointerEvents: "none",
      },
    },
    el(
      "div",
      {
        class: "card card--pad",
        style: {
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: "8px",
          padding: "30px 44px",
          border: "1.5px dashed var(--accent)",
          textAlign: "center",
        },
      },
      el("div", { html: icon("upload"), style: { width: "34px", height: "34px", color: "var(--accent)" } }),
      el("div", { style: { fontWeight: "600", fontSize: "14px" } }, "Отпустите PNG-чек"),
      el("div", { class: "small muted" }, "Номер и QR будут взяты из чека «Мой налог»"),
    ),
  );
  document.body.appendChild(dropOverlay);
  return dropOverlay;
}

function setDropVisible(visible, path = "") {
  const overlay = ensureDropOverlay();
  overlay.style.display = visible ? "grid" : "none";
  if (visible && path) {
    const label = overlay.querySelector(".small");
    if (label) label.textContent = path.split(/[\\/]/).pop() || path;
  }
}

async function initDragDrop() {
  try {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") {
        const path = payload.paths?.[0] ?? "";
        setDropVisible(true, path);
      } else if (payload.type === "drop") {
        setDropVisible(false);
        const path = payload.paths?.[0];
        if (path) handleDroppedPath(path);
      } else {
        setDropVisible(false);
      }
    });
  } catch {
    /* браузерный режим — drag & drop недоступен */
  }
}

/* ---------- горячие клавиши ---------- */

function initHotkeys() {
  document.addEventListener("keydown", (e) => {
    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;
    if (e.key.toLowerCase() === "n" && !e.shiftKey) {
      e.preventDefault();
      openEntryModal({ onSaved: () => window.dispatchEvent(new CustomEvent("cz:data-changed")) });
    } else if (e.key.toLowerCase() === "n" && e.shiftKey) {
      e.preventDefault();
      openWizard({ onSaved: () => window.dispatchEvent(new CustomEvent("cz:data-changed")) });
    } else if (e.key.toLowerCase() === "s") {
      e.preventDefault();
    }
  });
}

/* ---------- запуск ---------- */

async function boot() {
  applyTheme(localStorage.getItem("cz_theme") || "light");

  try {
    state.settings = await api.settings();
    state.sellers = await api.sellers(true);
    state.seller = await api.activeSeller();
    state.paths = await api.appPaths();
    applyTheme(state.settings.theme);
  } catch (e) {
    const view = document.getElementById("view");
    setHeader({ title: "Помощник самозанятого" });
    view.appendChild(
      el(
        "div",
        { class: "notice notice--danger" },
        el("div", { html: icon("alert") }),
        el(
          "div",
          {},
          "Не удалось запустить приложение. Открывайте его через Tauri: ",
          el("code", {}, "npm run tauri dev"),
          ". ",
          el("span", { class: "muted" }, String(e?.message ?? e)),
        ),
      ),
    );
    return;
  }

  mountShell();
  window.addEventListener("hashchange", renderRoute);
  window.addEventListener("cz:data-changed", () => renderRoute());
  window.addEventListener("cz:seller-changed", () => renderRoute());
  initHotkeys();
  await initDragDrop();
  await renderRoute();
}

boot();
