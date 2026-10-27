import * as pdfjs from "pdfjs-dist";
import workerSrc from "pdfjs-dist/build/pdf.worker.mjs?url";

pdfjs.GlobalWorkerOptions.workerSrc = workerSrc;

let renderToken = 0;

export async function renderPdf(container, base64, { padding = 0 } = {}) {
  const token = ++renderToken;
  container.innerHTML = "";
  const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
  const pdf = await pdfjs.getDocument({ data: bytes, disableFontFace: false }).promise;
  if (token !== renderToken) return 0;

  const width = Math.max(320, container.clientWidth - padding);
  const dpr = Math.min(window.devicePixelRatio || 1, 2);

  for (let n = 1; n <= pdf.numPages; n += 1) {
    const page = await pdf.getPage(n);
    if (token !== renderToken) return 0;
    const base = page.getViewport({ scale: 1 });
    const scale = width / base.width;
    const viewport = page.getViewport({ scale });

    const canvas = document.createElement("canvas");
    canvas.className = "wizard__canvas";
    canvas.width = Math.floor(viewport.width * dpr);
    canvas.height = Math.floor(viewport.height * dpr);
    canvas.style.width = `${Math.floor(viewport.width)}px`;
    canvas.style.height = `${Math.floor(viewport.height)}px`;
    if (n > 1) canvas.style.marginTop = "12px";

    const ctx = canvas.getContext("2d", { alpha: false });
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.scale(dpr, dpr);
    await page.render({ canvas, canvasContext: ctx, viewport }).promise;
    if (token !== renderToken) return 0;
    container.appendChild(canvas);
  }
  return pdf.numPages;
}

export function clearPdf(container) {
  renderToken += 1;
  container.innerHTML = "";
}
