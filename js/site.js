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
  var frame = lightbox && lightbox.querySelector(".lightbox-frame");
  var lightboxImg = frame && frame.querySelector("img");
  var scrim = lightbox && lightbox.querySelector(".lightbox-scrim");
  var controls = lightbox && lightbox.querySelector(".lightbox-ui");
  var shots = Array.prototype.slice.call(document.querySelectorAll(".gallery .shot img"));
  var lightboxFrom = null;
  var layout = null;
  var index = 0;
  var phase = "idle";
  var ignoreClick = false;
  var swipeX = null;
  var motionOk = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var OPEN_MS = 680;
  var CLOSE_MS = 520;
  var SLIDE_MS = 540;
  var REST = "translate(0px, 0px) scale(1)";
  var EASE_OUT = "cubic-bezier(0.16, 1, 0.3, 1)";
  var EASE_IN_OUT = "cubic-bezier(0.4, 0, 0.2, 1)";

  shots.forEach(function (shot) {
    var preload = new Image();
    preload.src = shot.currentSrc || shot.src;
  });

  function destRect() {
    var side = window.innerWidth < 720 ? 20 : 80;
    var top = 72;
    var bottom = 28;
    var maxW = Math.max(160, window.innerWidth - side * 2);
    var maxH = Math.max(90, window.innerHeight - top - bottom);
    var w = maxW;
    var h = w * 9 / 16;
    if (h > maxH) {
      h = maxH;
      w = h * 16 / 9;
    }
    return {
      left: (window.innerWidth - w) / 2,
      top: top + (maxH - h) / 2,
      width: w,
      height: h
    };
  }

  function placeFrame(rect) {
    layout = rect;
    frame.style.left = rect.left + "px";
    frame.style.top = rect.top + "px";
    frame.style.width = rect.width + "px";
    frame.style.height = rect.height + "px";
  }

  function pose(visual) {
    var scale = layout.width > 0 ? visual.width / layout.width : 1;
    var dx = visual.left - layout.left;
    var dy = visual.top - layout.top;
    return { transform: "translate(" + dx + "px," + dy + "px) scale(" + scale + ")" };
  }

  function dropEffects(el) {
    if (!el) return;
    el.getAnimations().forEach(function (anim) { anim.cancel(); });
  }

  function play(el, from, to, duration, easing) {
    dropEffects(el);
    var gen = (el._famPlay || 0) + 1;
    el._famPlay = gen;
    Object.keys(from).forEach(function (key) { el.style[key] = from[key]; });
    void el.offsetWidth;
    var anim = el.animate([from, to], { duration: duration, easing: easing, fill: "both" });
    function commit() {
      if (el._famPlay !== gen) return;
      Object.keys(to).forEach(function (key) { el.style[key] = to[key]; });
      anim.cancel();
    }
    anim.onfinish = commit;
    return anim;
  }

  function fade(el, to, duration) {
    if (!el) return null;
    var from = Number(getComputedStyle(el).opacity);
    if (isNaN(from)) from = to;
    return play(el, { opacity: from }, { opacity: to }, duration, "ease-out");
  }

  function showShot(thumb) {
    if (!lightbox || !frame || !lightboxImg || lightbox.open) return;
    index = Math.max(0, shots.indexOf(thumb));
    lightboxFrom = thumb;
    lightboxImg.src = thumb.currentSrc || thumb.src;
    lightboxImg.alt = thumb.alt;
    var from = thumb.getBoundingClientRect();
    lightbox.showModal();
    placeFrame(destRect());
    phase = "opening";
    if (!motionOk || from.width < 1) {
      frame.style.transform = "none";
      if (scrim) scrim.style.opacity = "1";
      if (controls) controls.style.opacity = "1";
      phase = "open";
      return;
    }
    var start = pose(from);
    var anim = play(frame, start, { transform: REST }, OPEN_MS, EASE_OUT);
    fade(scrim, 1, 480);
    fade(controls, 1, OPEN_MS);
    var commit = anim.onfinish;
    anim.onfinish = function (event) {
      if (commit) commit(event);
      if (phase === "opening") phase = "open";
    };
    window.setTimeout(function () {
      if (phase === "opening") phase = "open";
    }, OPEN_MS + 80);
  }

  function finishClose() {
    dropEffects(frame);
    dropEffects(scrim);
    dropEffects(controls);
    frame.style.transform = "";
    if (scrim) scrim.style.opacity = "";
    if (controls) controls.style.opacity = "";
    lightbox.close();
    phase = "idle";
  }

  function hideShot() {
    if (!lightbox || !lightbox.open || phase === "closing" || phase === "sliding") return;
    if (!motionOk || !lightboxFrom || !layout) {
      finishClose();
      return;
    }
    phase = "closing";
    var visual = frame.getBoundingClientRect();
    var back = pose(lightboxFrom.getBoundingClientRect());
    var anim = play(frame, pose(visual), back, CLOSE_MS, EASE_IN_OUT);
    fade(scrim, 0, 360);
    fade(controls, 0, 220);
    var finished = false;
    function done() {
      if (finished) return;
      finished = true;
      finishClose();
    }
    anim.onfinish = done;
    window.setTimeout(done, CLOSE_MS + 80);
  }

  function step(dir) {
    if (!lightbox || !lightbox.open || phase !== "open" || shots.length < 2) return;
    var next = (index + dir + shots.length) % shots.length;
    var thumb = shots[next];
    var src = thumb.currentSrc || thumb.src;
    index = next;
    lightboxFrom = thumb;
    if (!motionOk) {
      lightboxImg.src = src;
      lightboxImg.alt = thumb.alt;
      return;
    }
    phase = "sliding";
    var incoming = document.createElement("img");
    incoming.alt = thumb.alt;
    incoming.src = src;
    frame.appendChild(incoming);
    var outTo = dir > 0 ? "-100%" : "100%";
    var inFrom = dir > 0 ? "100%" : "-100%";
    var settled = false;
    lightboxImg.animate([
      { transform: "translateX(0%)" },
      { transform: "translateX(" + outTo + ")" }
    ], { duration: SLIDE_MS, easing: EASE_IN_OUT, fill: "forwards" });
    var enter = incoming.animate([
      { transform: "translateX(" + inFrom + ")" },
      { transform: "translateX(0%)" }
    ], { duration: SLIDE_MS, easing: EASE_IN_OUT, fill: "forwards" });
    function settle() {
      if (settled) return;
      settled = true;
      lightboxImg.src = src;
      lightboxImg.alt = thumb.alt;
      dropEffects(lightboxImg);
      lightboxImg.style.opacity = "";
      lightboxImg.style.transform = "";
      incoming.remove();
      if (phase === "sliding") phase = "open";
    }
    enter.onfinish = settle;
    window.setTimeout(settle, SLIDE_MS + 80);
  }

  document.querySelectorAll(".gallery .shot").forEach(function (button) {
    button.addEventListener("click", function () {
      var img = button.querySelector("img");
      if (img) showShot(img);
    });
  });
  if (lightbox) {
    lightbox.addEventListener("click", function (event) {
      if (ignoreClick) {
        ignoreClick = false;
        return;
      }
      var stepBtn = event.target.closest("[data-lightbox-step]");
      if (stepBtn) {
        step(Number(stepBtn.getAttribute("data-lightbox-step")));
        return;
      }
      if (event.target.closest("[data-lightbox-close]") || event.target === scrim || event.target === frame || event.target.tagName === "IMG") hideShot();
    });
    lightbox.addEventListener("cancel", function (event) {
      event.preventDefault();
      hideShot();
    });
    lightbox.addEventListener("keydown", function (event) {
      if (event.key === "ArrowRight") {
        event.preventDefault();
        step(1);
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        step(-1);
      }
    });
    frame.addEventListener("pointerdown", function (event) {
      if (event.button !== 0) return;
      swipeX = event.clientX;
    });
    frame.addEventListener("pointerup", function (event) {
      if (swipeX == null) return;
      var dx = event.clientX - swipeX;
      swipeX = null;
      if (Math.abs(dx) < 48) return;
      ignoreClick = true;
      step(dx < 0 ? 1 : -1);
    });
    window.addEventListener("resize", function () {
      if (!lightbox.open || phase !== "open") return;
      placeFrame(destRect());
    });
  }

  var theme = "blue";
  try { theme = localStorage.getItem("fam-theme") || "blue"; } catch (e) {}
  applyTheme(theme);
  applyLang(detectLang());
  fillCommands();
})();
