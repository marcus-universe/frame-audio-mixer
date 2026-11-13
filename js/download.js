// Downloads page: step 1 picks what to install, step 2 shows its guide.
// The step lives in the URL hash (#frame, #pc), so Back and shared links work.
(function () {
  var main = document.querySelector("main.download");
  var chooser = document.getElementById("chooser");
  if (!main || !chooser) return;

  var guides = { frame: document.getElementById("guide-frame"), pc: document.getElementById("guide-pc") };
  var motionOk = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var current = null;

  function viewFromHash() {
    var hash = location.hash.replace("#", "");
    return guides[hash] ? hash : null;
  }

  function replay(el) {
    el.classList.remove("enter");
    void el.offsetWidth;
    el.classList.add("enter");
  }

  function render(view) {
    chooser.hidden = view !== null;
    Object.keys(guides).forEach(function (key) { guides[key].hidden = key !== view; });
  }

  function focusAfter(view, previous) {
    if (view) {
      var title = guides[view].querySelector("h1");
      if (title) title.focus({ preventScroll: true });
    } else if (previous) {
      var card = chooser.querySelector('.pick[data-pick="' + previous + '"]');
      if (card) card.focus({ preventScroll: true });
    }
  }

  function show(view, animate) {
    if (view === current) return;
    var previous = current;
    current = view;
    // The card that morphs is the one being opened, or the one being returned to.
    var morph = view || previous;
    var crossGuide = view && previous;

    function swap() {
      render(view);
      window.scrollTo(0, 0);
    }

    var canMorph = animate && motionOk && document.startViewTransition && !crossGuide;
    if (canMorph) {
      main.classList.add("vt-" + morph);
      var transition = document.startViewTransition(swap);
      transition.finished.finally(function () { main.classList.remove("vt-" + morph); });
      transition.updateCallbackDone.then(function () { focusAfter(view, previous); });
      return;
    }
    swap();
    if (animate && motionOk) replay(view ? guides[view] : chooser);
    focusAfter(view, previous);
  }

  function go(view) {
    var target = view ? "#" + view : location.pathname + location.search;
    if (view && current) {
      // Guide to guide: keep one history entry per visit to the chooser.
      history.replaceState(history.state, "", target);
    } else if (view ? location.hash !== target : location.hash) {
      history.pushState(view ? { famFromChooser: true } : null, "", target);
    }
    show(view, true);
  }

  document.querySelectorAll("[data-pick]").forEach(function (button) {
    button.addEventListener("click", function () { go(button.getAttribute("data-pick")); });
  });

  document.querySelectorAll("[data-back]").forEach(function (button) {
    button.addEventListener("click", function () {
      // Step back in history when we came from the chooser, so Back does not loop.
      if (history.state && history.state.famFromChooser) history.back();
      else go(null);
    });
  });

  window.addEventListener("popstate", function () { show(viewFromHash(), true); });
  window.addEventListener("hashchange", function () { show(viewFromHash(), true); });

  // Windows / Linux switch, preset from the visitor's system.
  var osButtons = document.querySelectorAll("[data-os]");
  function setOs(os) {
    osButtons.forEach(function (b) { b.setAttribute("aria-pressed", b.getAttribute("data-os") === os ? "true" : "false"); });
    document.querySelectorAll("[data-os-panel]").forEach(function (panel) {
      panel.hidden = panel.getAttribute("data-os-panel") !== os;
    });
  }
  osButtons.forEach(function (b) {
    b.addEventListener("click", function () { setOs(b.getAttribute("data-os")); });
  });
  var platform = ((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "").toLowerCase();
  var isLinux = platform.indexOf("linux") >= 0 && !/android/i.test(navigator.userAgent);
  setOs(isLinux ? "linux" : "windows");

  // Opening the page on #frame or #pc goes straight to that guide.
  current = null;
  var initial = viewFromHash();
  if (initial) {
    render(initial);
    current = initial;
  }
})();
