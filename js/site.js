(function () {
  var FALLBACK = "https://frame-audio-mixer.marcus-universe.com/i";
  var LANGS = ["en", "ja", "sc", "de", "fr", "es", "it"];
  var root = document.documentElement;

  function installUrl() {
    if (location.protocol !== "http:" && location.protocol !== "https:") return FALLBACK;
    return new URL("i", location.href).href;
  }

  function command(kind) {
    var url = installUrl();
    var line = "curl -fsSL " + url + " | sh";
    if (kind === "autostart") return "curl -fsSL " + url + " | sh -s -- --autostart";
    if (kind === "uninstall") return "curl -fsSL " + url + " | sh -s -- --uninstall";
    return line;
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
    var pageTitle = page === "download" ? pack.dl_title : page === "imprint" ? pack.imprint_title : "";
    document.title = pageTitle ? pageTitle + " · Frame Audio Mixer" : pack.title;
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

  var lightbox = document.querySelector(".lightbox");
  var lightboxImg = lightbox && lightbox.querySelector("img");
  var lightboxFrom = null;
  var lightboxBusy = false;
  var motionOk = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  function shiftTo(from, to) {
    var scale = to.width > 0 ? from.width / to.width : 1;
    var dx = from.left - to.left;
    var dy = from.top - to.top;
    lightboxImg.style.transformOrigin = "top left";
    lightboxImg.style.transform = "translate(" + dx + "px," + dy + "px) scale(" + scale + ")";
  }

  function clearShift() {
    lightboxImg.style.transform = "";
    lightboxImg.style.transformOrigin = "";
  }

  function showShot(thumb) {
    if (!lightbox || !lightboxImg || lightboxBusy) return;
    lightboxFrom = thumb;
    lightboxImg.src = thumb.currentSrc || thumb.src;
    lightboxImg.alt = thumb.alt;
    lightbox.showModal();
    if (!motionOk) {
      lightbox.classList.add("is-shown");
      return;
    }
    var from = thumb.getBoundingClientRect();
    var to = lightboxImg.getBoundingClientRect();
    if (to.width < 1) {
      lightbox.classList.add("is-shown");
      return;
    }
    lightboxImg.style.transition = "none";
    shiftTo(from, to);
    lightbox.getBoundingClientRect();
    requestAnimationFrame(function () {
      lightboxImg.style.transition = "";
      lightbox.classList.add("is-shown");
      clearShift();
    });
  }

  function hideShot() {
    if (!lightbox || !lightbox.open || lightboxBusy) return;
    if (!motionOk || !lightboxFrom) {
      lightbox.classList.remove("is-shown");
      lightbox.close();
      return;
    }
    lightboxBusy = true;
    var from = lightboxImg.getBoundingClientRect();
    var to = lightboxFrom.getBoundingClientRect();
    lightbox.classList.remove("is-shown");
    shiftTo(to, from);
    var finished = false;
    function done(event) {
      if (finished) return;
      if (event && event.target !== lightboxImg) return;
      finished = true;
      lightboxImg.removeEventListener("transitionend", done);
      clearShift();
      lightbox.close();
      lightboxBusy = false;
    }
    lightboxImg.addEventListener("transitionend", done);
    window.setTimeout(done, 420);
  }

  document.querySelectorAll(".gallery .shot").forEach(function (button) {
    button.addEventListener("click", function () {
      var img = button.querySelector("img");
      if (img) showShot(img);
    });
  });
  if (lightbox) {
    lightbox.addEventListener("click", function (event) {
      if (event.target === lightbox || event.target === lightboxImg || event.target.closest("[data-lightbox-close]")) hideShot();
    });
    lightbox.addEventListener("cancel", function (event) {
      event.preventDefault();
      hideShot();
    });
  }

  var theme = "blue";
  try { theme = localStorage.getItem("fam-theme") || "blue"; } catch (e) {}
  applyTheme(theme);
  applyLang(detectLang());
  fillCommands();
})();
