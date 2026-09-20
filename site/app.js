/* causari.dev — no dependencies, no network. */
(function () {
  "use strict";

  var root = document.documentElement;

  // Theme: follow the system unless the visitor chose; the choice is kept locally.
  var STORE = "causari-theme";
  function apply(theme) {
    if (theme === "light" || theme === "dark") {
      root.setAttribute("data-theme", theme);
    } else {
      root.removeAttribute("data-theme");
    }
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) {
      var dark = theme === "dark" || (theme !== "light" && window.matchMedia("(prefers-color-scheme: dark)").matches);
      meta.setAttribute("content", dark ? "#0b0d10" : "#f5f4ef");
    }
  }
  var stored = null;
  try { stored = localStorage.getItem(STORE); } catch (e) { /* private mode */ }
  apply(stored);

  var toggle = document.getElementById("theme-toggle");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var current = root.getAttribute("data-theme");
      var systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      var isDark = current === "dark" || (!current && systemDark);
      var next = isDark ? "light" : "dark";
      apply(next);
      try { localStorage.setItem(STORE, next); } catch (e) { /* ignore */ }
    });
  }

  // Copy buttons: copy the text of the referenced <pre>, comments included.
  document.querySelectorAll("button.copy[data-copy]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var el = document.getElementById(btn.getAttribute("data-copy"));
      if (!el) return;
      var text = el.innerText.replace(/\s+$/, "") + "\n";
      var done = function () {
        var was = btn.textContent;
        btn.textContent = "copied";
        setTimeout(function () { btn.textContent = was; }, 1200);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(done, done);
      } else {
        var range = document.createRange();
        range.selectNodeContents(el);
        var sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
        try { document.execCommand("copy"); } catch (e) { /* ignore */ }
        sel.removeAllRanges();
        done();
      }
    });
  });

  var year = document.getElementById("year");
  if (year) year.textContent = String(new Date().getFullYear());
})();
