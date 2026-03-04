// Set contact link via JS to avoid Cloudflare email mangling
document.getElementById("contact-link").href =
  "mai" + "lto:nig" + "el@ryg" + "n.io";

// ===== RAINBOW LOGO — exact JS from working standalone version =====
var art =
  " /$$   /$$ /$$                     /$$\n| $$$ | $$|__/                    | $$\n| $$$$| $$ /$$  /$$$$$$   /$$$$$$ | $$\n| $$ $$ $$| $$ /$$__  $$ /$$__  $$| $$\n| $$  $$$$| $$| $$  \\ $$| $$$$$$$$| $$\n| $$\\  $$$| $$| $$  | $$| $$_____/| $$\n| $$ \\  $$| $$|  $$$$$$$|  $$$$$$$| $$\n|__/  \\__/|__/ \\____  $$ \\_______/|__/\n               /$$  \\ $$              \n              |  $$$$$$/              \n               \\______/               ";

var pre = document.getElementById("ascii-logo");
var lines = art.split("\n");
lines.forEach(function (line, lineIdx) {
  var lineEl = document.createElement("span");
  lineEl.style.display = "block";
  var chars = line.split("");
  chars.forEach(function (c, charIdx) {
    var span = document.createElement("span");
    span.className = "char";
    span.textContent = c === " " ? "\u00A0" : c;
    span.style.animationDelay = -(lineIdx * 0.15 + charIdx * 0.03) + "s";
    lineEl.appendChild(span);
  });
  pre.appendChild(lineEl);
});

// ===== FLOATING PARTICLES — exact JS from working standalone version =====
var colors = [
  "#ffb3ba",
  "#ffc8a2",
  "#ffe0a3",
  "#ffffba",
  "#c9ffcb",
  "#bae1ff",
  "#c4b7ff",
  "#ffb3de",
];
for (var i = 0; i < 30; i++) {
  var p = document.createElement("div");
  p.className = "particle";
  var size = Math.random() * 4 + 1.5;
  p.style.width = size + "px";
  p.style.height = size + "px";
  p.style.left = Math.random() * 100 + "vw";
  p.style.bottom = "-10px";
  p.style.background = colors[Math.floor(Math.random() * colors.length)];
  p.style.animationDuration = Math.random() * 8 + 6 + "s";
  p.style.animationDelay = Math.random() * 12 + "s";
  document.body.appendChild(p);
}

// ===== SCREENSHOT TABS =====
var tabs = document.querySelectorAll(".screenshot-tab");
var panels = document.querySelectorAll(".screenshot-panel");
tabs.forEach(function (tab) {
  tab.addEventListener("click", function () {
    tabs.forEach(function (t) {
      t.classList.remove("active");
    });
    panels.forEach(function (p) {
      p.classList.remove("active");
    });
    tab.classList.add("active");
    document
      .getElementById("panel-" + tab.getAttribute("data-tab"))
      .classList.add("active");
  });
});

// ===== SCROLL REVEAL =====
var reveals = document.querySelectorAll(".reveal");
var observer = new IntersectionObserver(
  function (entries) {
    entries.forEach(function (entry) {
      if (entry.isIntersecting) {
        entry.target.classList.add("visible");
      }
    });
  },
  { threshold: 0.1 },
);
reveals.forEach(function (el) {
  observer.observe(el);
});

// ===== HERO STAGGERED REVEAL — logo first, then rest =====
var heroReveals = document.querySelectorAll(".hero-reveal");
var delays = [600, 800, 1000, 1400];
setTimeout(function () {
  heroReveals.forEach(function (el, i) {
    setTimeout(
      function () {
        el.classList.add("visible");
      },
      delays[i] || 600 + i * 200,
    );
  });
}, 300);
