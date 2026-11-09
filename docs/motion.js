/* Scope for Linux website motion (design/05-motion-spec.md). Crafted tier, 8 items.
   Runs only when GSAP loaded; without it every element is already visible. */
(function () {
  "use strict";
  var g = window.gsap;
  if (!g || !window.ScrollTrigger) return;
  g.registerPlugin(window.ScrollTrigger);
  if (window.SplitText) g.registerPlugin(window.SplitText);
  var ST = window.ScrollTrigger;
  var html = document.documentElement;

  var mm = g.matchMedia();
  mm.add("(prefers-reduced-motion: no-preference)", function () {
    html.classList.add("motion");
    var cleanups = [];

    /* 5. Lenis on the GSAP ticker, fine pointers only */
    var lenis = null;
    if (window.Lenis && matchMedia("(pointer: fine)").matches) {
      lenis = new window.Lenis({ lerp: 0.1, smoothWheel: true, syncTouch: false });
      lenis.on("scroll", ST.update);
      var tick = function (t) { lenis.raf(t * 1000); };
      g.ticker.add(tick);
      g.ticker.lagSmoothing(0);
      var onModal = function (e) { if (e.detail) lenis.stop(); else lenis.start(); };
      document.addEventListener("scope:modal", onModal);
      var onAnchor = function (e) {
        var a = e.target.closest('a[href^="#"]');
        if (!a || a.getAttribute("href").length < 2) return;
        var target = document.querySelector(a.getAttribute("href"));
        if (!target) return;
        e.preventDefault();
        lenis.scrollTo(target, { offset: -72 });
        history.replaceState(null, "", a.getAttribute("href"));
      };
      document.addEventListener("click", onAnchor);
      cleanups.push(function () {
        g.ticker.remove(tick);
        document.removeEventListener("scope:modal", onModal);
        document.removeEventListener("click", onAnchor);
        lenis.destroy();
      });
    }

    /* 1. Reveals */
    g.utils.toArray("[data-reveal]").forEach(function (el) {
      var kids = el.hasAttribute("data-reveal-stagger") ? el.children : null;
      g.set(el, { opacity: 1 });
      g.from(kids || el, {
        y: 24, opacity: 0, duration: 0.7, ease: "expo.out", stagger: kids ? 0.06 : 0,
        scrollTrigger: { trigger: el, start: "top 85%", once: true }
      });
    });

    var fontsReady = document.fonts && document.fonts.ready ? document.fonts.ready : Promise.resolve();
    fontsReady.then(function () {
      /* 6. SplitText masked line reveals */
      var splits = {};
      g.set("[data-split]", { opacity: 1 });
      if (window.SplitText) {
        g.utils.toArray("[data-split]").forEach(function (el) {
          el.setAttribute("aria-label", el.textContent.trim());
          var s = window.SplitText.create(el, { type: "lines", mask: "lines", linesClass: "split-line" });
          s.lines.forEach(function (l) { l.setAttribute("aria-hidden", "true"); });
          splits[el.id] = s;
          if (el.closest(".hero")) return;
          g.from(s.lines, { yPercent: 110, duration: 0.9, ease: "expo.out", stagger: 0.08, scrollTrigger: { trigger: el, start: "top 85%", once: true } });
        });
      }

      /* 7. Hero load */
      var heroLines = splits["hero-title"] ? splits["hero-title"].lines : document.querySelectorAll("#hero-title");
      var heroBits = document.querySelectorAll(".hero [data-hero]");
      g.set(heroBits, { opacity: 1 });
      var tl = g.timeline({ defaults: { ease: "expo.out" } });
      tl.from(".site-header", { y: -8, opacity: 0, duration: 0.6 }, 0)
        .from(heroBits[0], { opacity: 0, duration: 0.5 }, 0.06)
        .from(heroLines, { yPercent: 110, duration: 1, stagger: 0.08 }, 0.12)
        .from(heroBits[1], { y: 12, opacity: 0, duration: 0.7 }, 0.42)
        .from([heroBits[2], heroBits[3]], { y: 12, opacity: 0, duration: 0.7, stagger: 0.06 }, 0.52)
        .from("[data-stage]", { y: 40, opacity: 0, duration: 1.1 }, 0.6);

      ST.refresh();
    });

    window.addEventListener("load", function () { ST.refresh(); }, { once: true });

    return function () { cleanups.forEach(function (fn) { fn(); }); html.classList.remove("motion"); };
  });

  /* 8. Signature: the plan builds as you scroll (desktop only) */
  mm.add("(prefers-reduced-motion: no-preference) and (min-width: 961px)", function () {
    var stage = document.querySelector("[data-plan-stage]");
    var sheet = document.querySelector("[data-sheet]");
    if (!stage || !sheet) return;
    var steps = g.utils.toArray("[data-steps] .step");
    var groups = g.utils.toArray("[data-group]", sheet);
    var stamp = sheet.querySelector("[data-stamp]");
    var confirmPill = sheet.querySelector("[data-confirm-pill]");
    document.body.classList.add("js-plan");

    function setStep(n) {
      steps.forEach(function (s, i) {
        s.classList.toggle("is-active", i === n - 1);
        s.classList.toggle("is-done", i < n - 1);
      });
      if (confirmPill) confirmPill.classList.toggle("is-pressed", n >= 3);
    }

    // Group 01 (the plan itself) is on screen from the start; its stamp lands, then 02–04 build.
    groups.slice(1).forEach(function (gr) { g.set(gr.querySelectorAll("[data-line], .gl"), { opacity: 0, y: 10 }); });
    if (stamp) g.set(stamp, { scale: 1.25, rotate: -8, opacity: 0 });

    var tl = g.timeline({
      defaults: { ease: "none" },
      scrollTrigger: {
        trigger: stage, start: "top top+=64", end: "+=220%", pin: true, scrub: 0.6, anticipatePin: 1,
        onUpdate: function (self) {
          var p = self.progress;
          setStep(p < 0.25 ? 1 : p < 0.45 ? 2 : p < 0.75 ? 3 : 4);
        }
      }
    });
    var spans = [[0, 0.25], [0.25, 0.45], [0.45, 0.75], [0.75, 1]];
    if (stamp) tl.to(stamp, { scale: 1, rotate: -3, opacity: 1, duration: 0.08, ease: "power3.out" }, 0.08);
    groups.slice(1).forEach(function (gr, k) {
      var items = gr.querySelectorAll(".gl, [data-line]");
      var span = spans[k + 1], len = span[1] - span[0];
      var each = (len * 0.8) / items.length;
      Array.prototype.forEach.call(items, function (it, j) {
        tl.to(it, { opacity: 1, y: 0, duration: each * 0.9, ease: "power2.out" }, span[0] + j * each);
      });
    });
    tl.to({}, { duration: 0.02 }, 1);
    setStep(1);

    return function () {
      document.body.classList.remove("js-plan");
      steps.forEach(function (s) { s.classList.remove("is-active", "is-done"); });
      if (confirmPill) confirmPill.classList.remove("is-pressed");
    };
  });
})();
