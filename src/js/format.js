const MONTHS = [
  "января", "февраля", "марта", "апреля", "мая", "июня",
  "июля", "августа", "сентября", "октября", "ноября", "декабря",
];

const MONTHS_NOM = [
  "Январь", "Февраль", "Март", "Апрель", "Май", "Июнь",
  "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь",
];

const MONTHS_SHORT = ["янв", "фев", "мар", "апр", "май", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"];

export function todayIso() {
  const d = new Date();
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function nowYear() {
  return new Date().getFullYear();
}

export function nowMonth() {
  return new Date().getMonth() + 1;
}

export function money(kop) {
  const sign = kop < 0 ? "−" : "";
  const abs = Math.abs(Math.round(kop));
  const rub = Math.floor(abs / 100);
  const cents = String(abs % 100).padStart(2, "0");
  return `${sign}${group(rub)},${cents}`;
}

export function group(n) {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}

export function rub(kop) {
  return `${money(kop)} ₽`;
}

export function moneyCompact(kop) {
  const value = Math.round(kop) / 100;
  if (Math.abs(value) >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(value % 1_000_000 === 0 ? 0 : 1).replace(".", ",")} млн`;
  }
  if (Math.abs(value) >= 10_000) {
    return `${Math.round(value / 1000)} тыс.`;
  }
  return money(kop);
}

export function parseMoney(input) {
  if (typeof input === "number") return Math.round(input * 100);
  let s = String(input ?? "").trim();
  if (!s) return null;
  s = s.replace(/\s|\u00a0/g, "").replace(/₽|руб\.?/gi, "").replace(",", ".");
  s = s.replace(/[^\d.\-]/g, "");
  if (!s || s === "-" || s === ".") return null;
  const value = Number(s);
  if (!Number.isFinite(value)) return null;
  return Math.round(value * 100);
}

export function moneyInput(kop) {
  if (kop === null || kop === undefined) return "";
  return money(kop);
}

export function parseQty(input) {
  let s = String(input ?? "").trim().replace(/\s/g, "").replace(",", ".");
  if (!s) return null;
  const value = Number(s);
  if (!Number.isFinite(value) || value <= 0) return null;
  return Math.round(value * 1000);
}

export function qtyText(milli) {
  if (milli % 1000 === 0) return String(milli / 1000);
  return (milli / 1000).toFixed(3).replace(/0+$/, "").replace(/\.$/, "").replace(".", ",");
}

export function dateRu(iso) {
  if (!iso || iso.length < 10) return iso || "";
  return `${iso.slice(8, 10)}.${iso.slice(5, 7)}.${iso.slice(0, 4)}`;
}

export function dateFull(iso) {
  if (!iso || iso.length < 10) return iso || "";
  const day = Number(iso.slice(8, 10));
  const month = Number(iso.slice(5, 7)) - 1;
  return `${day} ${MONTHS[month] ?? ""} ${iso.slice(0, 4)}`;
}

export function monthShort(m) {
  return MONTHS_SHORT[m - 1] ?? "";
}

export function monthLabel(year, month) {
  return `${MONTHS_NOM[month - 1] ?? ""} ${year}`;
}

export function monthLabelLower(year, month) {
  return `${MONTHS[month - 1] ?? ""} ${year}`;
}

export function plural(n, one, few, many) {
  const abs = Math.abs(n) % 100;
  if (abs >= 11 && abs <= 14) return many;
  switch (abs % 10) {
    case 1:
      return one;
    case 2:
    case 3:
    case 4:
      return few;
    default:
      return many;
  }
}

export function itemsWord(n) {
  return plural(n, "наименование", "наименования", "наименований");
}

export function entriesWord(n) {
  return plural(n, "операция", "операции", "операций");
}

export function daysWord(n) {
  return plural(n, "день", "дня", "дней");
}

export function payerLabel(type) {
  return type === "organization" ? "Юрлицо" : "Физлицо";
}

export function initials(name) {
  const parts = String(name ?? "").trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "";
  if (parts.length === 1) return parts[0];
  const i1 = parts[1][0] ?? "";
  const i2 = parts[2]?.[0] ?? "";
  return `${parts[0]} ${i1}.${i2 ? ` ${i2}.` : ""}`;
}

export function shortName(name, max = 28) {
  const s = String(name ?? "");
  return s.length > max ? `${s.slice(0, max - 1)}…` : s;
}

export function daysUntil(iso) {
  const target = new Date(`${iso}T00:00:00`);
  const now = new Date();
  now.setHours(0, 0, 0, 0);
  return Math.round((target - now) / 86400000);
}

export function isoDate(year, month, day) {
  const p = (n) => String(n).padStart(2, "0");
  return `${year}-${p(month)}-${p(day)}`;
}
