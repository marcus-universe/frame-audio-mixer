(function () {
  var FALLBACK = "https://marcus-universe.github.io/frame-audio-mixer/i";
  var LANGS = ["en", "ja", "sc", "de", "fr", "es", "it"];
  var root = document.documentElement;

  function installUrl() {
    if (location.protocol !== "http:" && location.protocol !== "https:") return FALLBACK;
    return new URL("i", location.href).href;
  }

  function command(kind) {
    var url = installUrl();
    if (kind === "autostart") return "curl -fsSL " + url + " | sh -s -- --autostart";
    if (kind === "uninstall") return "curl -fsSL " + url + " | sh -s -- --uninstall";
    return "curl -fsSL " + url + " | sh";
  }

  function fillCommands() {
    document.querySelectorAll("[data-install]").forEach(function (el) {
      el.textContent = command(el.getAttribute("data-install"));
    });
  }

  function langPack(code) {
    return window.FAM_I18N[code] || window.FAM_I18N.en;
  }

  function applyLang(code) {
    if (LANGS.indexOf(code) < 0) code = "en";
    var pack = langPack(code);
    root.lang = code === "sc" ? "zh-Hans" : code;
    document.querySelectorAll("[data-i18n]").forEach(function (el) {
      var key = el.getAttribute("data-i18n");
      if (pack[key]) el.textContent = pack[key];
    });
    document.querySelectorAll("[data-i18n-alt]").forEach(function (el) {
      var key = el.getAttribute("data-i18n-alt");
      if (pack[key]) el.alt = pack[key];
    });
    document.querySelectorAll("[data-i18n-aria]").forEach(function (el) {
      var key = el.getAttribute("data-i18n-aria");
      if (pack[key]) el.setAttribute("aria-label", pack[key]);
    });
    var page = document.body.getAttribute("data-page");
    document.title = page === "download" ? pack.dl_title + " · Frame Audio Mixer" : pack.title;
    var select = document.getElementById("lang");
    if (select && select.value !== code) select.value = code;
    try { localStorage.setItem("fam-lang", code); } catch (e) {}
  }

  function applyTheme(name) {
    if (name !== "pink") name = "blue";
    root.setAttribute("data-theme", name);
    document.querySelectorAll("[data-logo]").forEach(function (img) {
      var kind = img.getAttribute("data-logo");
      img.src = "assets/logo/logo_" + kind + "_" + name + ".svg";
    });
    document.querySelectorAll(".themes button").forEach(function (button) {
      button.setAttribute("aria-pressed", button.getAttribute("data-theme-set") === name ? "true" : "false");
    });
    try { localStorage.setItem("fam-theme", name); } catch (e) {}
  }

  function detectLang() {
    var stored = "";
    try { stored = localStorage.getItem("fam-lang") || ""; } catch (e) {}
    if (LANGS.indexOf(stored) >= 0) return stored;
    var nav = (navigator.language || "en").toLowerCase();
    if (nav.indexOf("ja") === 0) return "ja";
    if (nav.indexOf("zh") === 0) return "sc";
    if (nav.indexOf("de") === 0) return "de";
    if (nav.indexOf("fr") === 0) return "fr";
    if (nav.indexOf("es") === 0) return "es";
    if (nav.indexOf("it") === 0) return "it";
    return "en";
  }

  var select = document.getElementById("lang");
  if (select) {
    LANGS.forEach(function (code) {
      var option = document.createElement("option");
      option.value = code;
      option.textContent = langPack(code).lang_name;
      select.appendChild(option);
    });
    select.addEventListener("change", function () { applyLang(select.value); });
  }

  document.querySelectorAll(".themes button").forEach(function (button) {
    button.addEventListener("click", function () {
      applyTheme(button.getAttribute("data-theme-set"));
    });
  });

  document.querySelectorAll("[data-copy]").forEach(function (button) {
    button.addEventListener("click", function () {
      var code = button.parentElement.querySelector("code");
      if (!code || !navigator.clipboard) return;
      navigator.clipboard.writeText(code.textContent).then(function () {
        var pack = langPack(root.lang === "zh-Hans" ? "sc" : root.lang);
        var previous = button.textContent;
        button.textContent = pack.copied || "Copied";
        window.setTimeout(function () { button.textContent = previous; }, 1600);
      });
    });
  });

  var theme = "blue";
  try { theme = localStorage.getItem("fam-theme") || "blue"; } catch (e) {}
  applyTheme(theme);
  applyLang(detectLang());
  fillCommands();
})();
