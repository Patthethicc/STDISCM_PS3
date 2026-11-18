// Client-side behaviour for the server-rendered GUI:
//  - hover a card: play its 10-second preview from the start; stop when the mouse leaves
//  - click a card: open the full video in a modal
//  - every 3 seconds: re-fetch the grid so new uploads appear
(() => {
  const REFRESH_MS = 3000;
  const modal = document.getElementById("modal");
  const modalVideo = document.getElementById("modal-video");
  const modalTitle = document.getElementById("modal-title");
  let lastGrid = document.getElementById("grid-root")?.outerHTML ?? "";

  const cardOf = (el) => el.closest && el.closest(".card");

  function startPreview(card) {
    const clip = card.querySelector("video.clip");
    if (!clip) return;
    card.classList.add("playing");
    clip.currentTime = 0;
    clip.play().catch(() => {});
  }

  function stopPreview(card) {
    const clip = card.querySelector("video.clip");
    card.classList.remove("playing");
    if (!clip) return;
    clip.pause();
    clip.currentTime = 0;
  }

  // mouseenter/leave don't bubble, so listen in the capture phase.
  // This keeps working after the grid is replaced by auto-refresh.
  document.addEventListener("mouseenter", (e) => {
    if (e.target.classList?.contains("card")) startPreview(e.target);
  }, true);
  document.addEventListener("mouseleave", (e) => {
    if (e.target.classList?.contains("card")) stopPreview(e.target);
  }, true);

  // Fallback when there is no 10s clip: loop the first 10 seconds of the full video.
  document.addEventListener("timeupdate", (e) => {
    const v = e.target;
    const limit = Number(v.dataset?.limit);
    if (limit && v.currentTime >= limit) v.currentTime = 0;
  }, true);

  function openModal(card) {
    stopPreview(card);
    modalTitle.textContent = card.dataset.title;
    modalVideo.src = card.dataset.src;
    modal.hidden = false;
    modalVideo.play().catch(() => {});
  }

  function closeModal() {
    modalVideo.pause();
    modalVideo.removeAttribute("src");
    modalVideo.load();
    modal.hidden = true;
  }

  document.addEventListener("click", (e) => {
    const card = cardOf(e.target);
    if (card) return openModal(card);
    if (e.target.id === "modal-close" || e.target === modal) closeModal();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !modal.hidden) closeModal();
    const card = cardOf(e.target);
    if (card && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); openModal(card); }
  });

  async function refresh() {
    // Don't swap the grid while a preview is playing or the player is open.
    if (!modal.hidden || document.querySelector(".card.playing")) return;
    try {
      const res = await fetch("/fragments/grid", { cache: "no-store" });
      if (!res.ok) return;
      const html = await res.text();
      if (html === lastGrid) return;
      lastGrid = html;
      document.getElementById("grid-root").outerHTML = html;
    } catch { /* GUI server unreachable; try again next tick */ }
  }
  setInterval(refresh, REFRESH_MS);
})();
