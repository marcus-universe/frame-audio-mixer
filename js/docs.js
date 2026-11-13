// Docs page: picks the English or German article, builds the contents from
// its headings, and lights the contents wire up to the chapter in view.
(function () {
  var article = document.getElementById("doc");
  var german = document.getElementById("doc-de");
  var list = document.querySelector("[data-toc]");
  var toggle = document.querySelector(".toc-toggle");
  var now = document.querySelector("[data-toc-now]");
  var tocTitle = document.querySelector("[data-toc-title]");
  var note = document.querySelector(".lang-note");
  if (!article || !list) return;

  var english = document.createElement("template");
  english.innerHTML = article.innerHTML;
  var shown = "en";
  var links = [];
  var headings = [];
  var current = -1;
  var observer = null;

  var COPY = { en: ["Copy", "Copied"], de: ["Kopieren", "Kopiert"] };
  var TITLE = { en: "On this page", de: "Auf dieser Seite" };

  function siteLang() {
    var lang = document.documentElement.lang || "en";
    return lang === "zh-Hans" ? "sc" : lang;
  }

  function build() {
    list.innerHTML = "";
    links = [];
    headings = Array.prototype.slice.call(article.querySelectorAll("section > h2[id], section > h3[id]"));
    var parent = null;
    headings.forEach(function (heading) {
      var item = document.createElement("li");
      var link = document.createElement("a");
      link.href = "#" + heading.id;
      link.textContent = heading.textContent;
      item.appendChild(link);
      if (heading.tagName === "H3") {
        item.className = "sub";
        // Sub-chapters only show for the chapter they belong to, so the wire
        // stays short enough to read at a glance.
        item.setAttribute("data-parent", parent ? parent.id : "");
      } else {
        parent = heading;
      }
      list.appendChild(item);
      links.push(link);
    });
    tocTitle.textContent = TITLE[shown];
    current = -1;
    watch();
    mark(firstVisible());
  }

  function firstVisible() {
    var line = window.innerWidth <= 960 ? 96 : 40;
    var found = 0;
    headings.forEach(function (heading, index) {
      if (heading.getBoundingClientRect().top - line <= 0) found = index;
    });
    return found;
  }

  function mark(index) {
    if (index === current || !links[index]) return;
    current = index;
    var chapter = headings[index].tagName === "H2" ? headings[index].id : links[index].parentElement.getAttribute("data-parent");
    links.forEach(function (link, i) {
      var item = link.parentElement;
      if (i === index) link.setAttribute("aria-current", "location");
      else link.removeAttribute("aria-current");
      item.classList.toggle("passed", i < index && !item.classList.contains("sub"));
      if (item.classList.contains("sub")) item.hidden = item.getAttribute("data-parent") !== chapter;
    });
    var active = links[index];
    var top = active.parentElement.offsetTop + active.offsetTop + 12;
    list.style.setProperty("--level", Math.max(0, top - 12) + "px");
    if (now) {
      var main = headings[index].tagName === "H2" ? headings[index] : document.getElementById(chapter);
      now.textContent = main ? main.textContent : active.textContent;
    }
  }

  function watch() {
    if (observer) observer.disconnect();
    if (!("IntersectionObserver" in window)) return;
    observer = new IntersectionObserver(function () { mark(firstVisible()); }, {
      rootMargin: "0px 0px -55% 0px",
      threshold: [0, 1]
    });
    headings.forEach(function (heading) { observer.observe(heading); });
  }

  function show(lang) {
    var want = lang === "de" && german ? "de" : "en";
    var others = ["ja", "sc", "fr", "es", "it"].indexOf(lang) >= 0;
    if (note) note.hidden = !others;
    if (want === shown && list.children.length) return;
    // The English article is already in the page; only a switch swaps it.
    var swap = want !== shown;
    shown = want;
    if (swap) article.innerHTML = (want === "de" ? german : english).innerHTML;
    article.lang = want;
    document.querySelectorAll("[data-doc-copy]").forEach(function (button) {
      button.textContent = COPY[want][0];
    });
    build();
    if (swap && location.hash) {
      var target = document.getElementById(location.hash.slice(1));
      if (target) target.scrollIntoView();
    }
  }

  article.addEventListener("click", function (event) {
    var button = event.target.closest("[data-doc-copy]");
    if (!button) return;
    var code = button.parentElement.querySelector("code");
    if (!code || !navigator.clipboard) return;
    navigator.clipboard.writeText(code.textContent).then(function () {
      button.textContent = COPY[shown][1];
      window.setTimeout(function () { button.textContent = COPY[shown][0]; }, 1600);
    });
  });

  function setOpen(open) {
    if (toggle) toggle.setAttribute("aria-expanded", open ? "true" : "false");
  }

  if (toggle) {
    toggle.addEventListener("click", function () {
      setOpen(toggle.getAttribute("aria-expanded") !== "true");
    });
  }

  list.addEventListener("click", function (event) {
    if (event.target.closest("a")) setOpen(false);
  });

  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape" && toggle && toggle.getAttribute("aria-expanded") === "true") {
      setOpen(false);
      toggle.focus();
    }
  });

  document.addEventListener("click", function (event) {
    if (toggle && !event.target.closest(".toc")) setOpen(false);
  });

  window.addEventListener("scroll", function () { mark(firstVisible()); }, { passive: true });
  window.addEventListener("resize", function () { var index = current; current = -1; mark(index < 0 ? 0 : index); });
  document.addEventListener("fam:lang", function (event) { show(event.detail); });

  show(siteLang());
})();
