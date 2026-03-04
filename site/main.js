// Set contact link via JS to avoid automated email scraping
document.getElementById("contact-link").href =
  "mai" + "lto:nig" + "el@ryg" + "n.io";

// ===== RAINBOW LOGO — renders ASCII art char-by-char with staggered animation =====
const art =
  " /$$   /$$ /$$                     /$$\n| $$$ | $$|__/                    | $$\n| $$$$| $$ /$$  /$$$$$$   /$$$$$$ | $$\n| $$ $$ $$| $$ /$$__  $$ /$$__  $$| $$\n| $$  $$$$| $$| $$  \\ $$| $$$$$$$$| $$\n| $$\\  $$$| $$| $$  | $$| $$_____/| $$\n| $$ \\  $$| $$|  $$$$$$$|  $$$$$$$| $$\n|__/  \\__/|__/ \\____  $$ \\_______/|__/\n               /$$  \\ $$              \n              |  $$$$$$/              \n               \\______/               ";

const pre = document.getElementById("ascii-logo");
const lines = art.split("\n");
lines.forEach((line, lineIdx) => {
  const lineEl = document.createElement("span");
  lineEl.style.display = "block";
  line.split("").forEach((c, charIdx) => {
    const span = document.createElement("span");
    span.className = "char";
    span.textContent = c === " " ? "\u00A0" : c;
    span.style.animationDelay = `${-(lineIdx * 0.15 + charIdx * 0.03)}s`;
    lineEl.appendChild(span);
  });
  pre.appendChild(lineEl);
});

// ===== FLOATING PARTICLES — pastel circles that drift upward =====
const colors = [
  "#ffb3ba",
  "#ffc8a2",
  "#ffe0a3",
  "#ffffba",
  "#c9ffcb",
  "#bae1ff",
  "#c4b7ff",
  "#ffb3de",
];
for (let i = 0; i < 30; i++) {
  const p = document.createElement("div");
  p.className = "particle";
  const size = Math.random() * 4 + 1.5;
  p.style.width = `${size}px`;
  p.style.height = `${size}px`;
  p.style.left = `${Math.random() * 100}vw`;
  p.style.background = colors[Math.floor(Math.random() * colors.length)];
  p.style.animationDuration = `${Math.random() * 8 + 6}s`;
  p.style.animationDelay = `${Math.random() * 12}s`;
  document.body.appendChild(p);
}

// ===== SCREENSHOT TABS — accessible tab switching with ARIA updates =====
const tabs = document.querySelectorAll(".screenshot-tab");
const panels = document.querySelectorAll(".screenshot-panel");
tabs.forEach((tab) => {
  tab.addEventListener("click", () => {
    tabs.forEach((t) => {
      t.classList.remove("active");
      t.setAttribute("aria-selected", "false");
    });
    panels.forEach((p) => {
      p.classList.remove("active");
    });
    tab.classList.add("active");
    tab.setAttribute("aria-selected", "true");
    document
      .getElementById(`panel-${tab.getAttribute("data-tab")}`)
      .classList.add("active");
  });
});

// ===== SCROLL REVEAL =====
const reveals = document.querySelectorAll(".reveal");
const observer = new IntersectionObserver(
  (entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add("visible");
      }
    });
  },
  { threshold: 0.1 },
);
reveals.forEach((el) => {
  observer.observe(el);
});

// ===== HERO STAGGERED REVEAL — tagline, CTA, and scroll hint fade in after logo CSS animation =====
const heroReveals = document.querySelectorAll(".hero-reveal");
const delays = [600, 800, 1000, 1400];
setTimeout(() => {
  heroReveals.forEach((el, i) => {
    setTimeout(() => {
      el.classList.add("visible");
    }, delays[i] ?? (600 + i * 200));
  });
}, 300);
