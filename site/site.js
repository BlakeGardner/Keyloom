// Keyloom website: the hero's headline typing itself over a field of keys,
// the recording's pause and expand buttons, the feature tiles coming in as
// they scroll into view and their illustrations' loops, and the install
// section's distribution and version tabs with their copy buttons. Without
// scripting, the headline is plain text, the recording still loops at the
// page's width, every tile keeps its still illustration and every install
// block shows in order. When the system asks for reduced motion nothing
// moves on its own.
(function () {
  "use strict";

  var still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var observes = "IntersectionObserver" in window;

  function shuffle(list) {
    for (var i = list.length - 1; i > 0; i--) {
      var j = Math.floor(Math.random() * (i + 1));
      var swap = list[i];
      list[i] = list[j];
      list[j] = swap;
    }
    return list;
  }

  function pick(list) {
    return list[Math.floor(Math.random() * list.length)];
  }

  // ---- The key field ----------------------------------------------------

  // Loose keys across the top of the hero. The ones under the column the
  // eyebrow, the headline, the lead and the button take are the faint
  // kind, so the words stay easy to read; a phone gets smaller keys. A key
  // goes down as the pointer passes over it or a finger touches it (--p,
  // from 0 to 1, set here and eased by the stylesheet), the headline
  // presses a key with each letter it types, and a key is pressed on its
  // own now and then while the hero is on screen. Each key hangs on a
  // spring of its own, so when the page starts or stops scrolling the keys
  // lag behind or overshoot, each by its own weight, and bounce back.

  function keyField(hero, shot) {
    var REACH = 150;
    var layer = document.createElement("div");
    layer.className = "keyfield";
    layer.setAttribute("aria-hidden", "true");
    hero.insertBefore(layer, hero.firstChild);

    var keys = [];
    var byLabel = {};
    var width = 0;
    var height = 0;

    // The widest the text and the button get, measured from their lines of
    // text rather than their boxes, which stretch to the column's width.
    function textWidth() {
      var left = Infinity;
      var right = -Infinity;
      Array.prototype.forEach.call(hero.querySelector(".wrap").children, function (child) {
        if (child === shot) {
          return;
        }
        var walker = document.createTreeWalker(child, NodeFilter.SHOW_TEXT);
        var text;
        while ((text = walker.nextNode())) {
          var range = document.createRange();
          range.selectNodeContents(text);
          Array.prototype.forEach.call(range.getClientRects(), function (rect) {
            if (rect.width) {
              left = Math.min(left, rect.left);
              right = Math.max(right, rect.right);
            }
          });
        }
      });
      return (right > left ? right - left : 0) + 40;
    }

    function layout() {
      var nextWidth = getComputedStyle(layer).display === "none" ? 0 : hero.clientWidth;
      var nextHeight = Math.max(
        0,
        shot.getBoundingClientRect().top - hero.getBoundingClientRect().top - 24
      );
      if (Math.abs(nextWidth - width) < 8 && Math.abs(nextHeight - height) < 24) {
        return;
      }
      width = nextWidth;
      height = nextHeight;
      layer.style.height = height + "px";
      layer.textContent = "";
      keys = [];
      byLabel = {};

      var CELL = width < 640 ? 48 : 56;
      var SIZE = width < 640 ? 30 : 34;
      var columns = Math.floor(width / CELL);
      var rows = Math.floor(height / CELL);
      if (columns < 1 || rows < 1) {
        return;
      }
      var cells = [];
      for (var row = 0; row < rows; row++) {
        for (var column = 0; column < columns; column++) {
          cells.push({ column: column, row: row });
        }
      }
      shuffle(cells);
      // The headline's letters first, so each one has a key to press.
      var labels = shuffle("YOURKEBADTHWNI".split("")).concat(
        shuffle("CFGJLMPQSVXZ".split("")),
        shuffle(["Esc", "Fn", "F8", "Alt", "Tab"])
      );
      var inset = (width - columns * CELL) / 2;
      var textEdge = (width - textWidth()) / 2;
      var count = Math.round(cells.length * 0.55);
      for (var i = 0; i < count; i++) {
        var cell = cells[i];
        var label = labels[i % labels.length];
        var x = inset + cell.column * CELL + (CELL - SIZE) / 2 + (Math.random() - 0.5) * 16;
        var y = cell.row * CELL + (CELL - SIZE) / 2 + (Math.random() - 0.5) * 16 + 6;
        var under = x + SIZE > textEdge && x < width - textEdge;
        var key = document.createElement("span");
        key.className = under ? "key key--under" : "key";
        key.textContent = label;
        key.style.left = Math.round(x) + "px";
        key.style.top = Math.round(y) + "px";
        key.style.setProperty("--r", ((Math.random() - 0.5) * 10).toFixed(1) + "deg");
        layer.appendChild(key);
        // A spring's weight sets how far it swings, its pace how fast it
        // bounces (13 to 17 radians a second, two or three bounces a
        // second), and its give how quickly the bouncing dies down.
        var pace = 13 + Math.random() * 4;
        keys.push({
          el: key,
          x: x + SIZE / 2,
          y: y + SIZE / 2,
          p: "0",
          weight: 0.6 + Math.random() * 0.8,
          spring: pace * pace,
          give: 2 * (0.18 + Math.random() * 0.1) * pace,
          tilt: (Math.random() < 0.5 ? -1 : 1) * (0.5 + Math.random() * 0.5),
          off: 0,
          speed: 0
        });
        var id = label.toUpperCase();
        (byLabel[id] = byLabel[id] || []).push(key);
      }
    }

    var pointerX = -1e4;
    var pointerY = -1e4;
    var pending = false;

    function follow() {
      pending = false;
      var rect = layer.getBoundingClientRect();
      var x = pointerX - rect.left;
      var y = pointerY - rect.top;
      keys.forEach(function (key) {
        var dx = key.x - x;
        var dy = key.y - y;
        var near = 1 - Math.sqrt(dx * dx + dy * dy) / REACH;
        var p = near > 0 ? (near * near).toFixed(2) : "0";
        if (p !== key.p) {
          key.p = p;
          key.el.style.setProperty("--p", p);
        }
      });
    }

    function moved(x, y) {
      pointerX = x;
      pointerY = y;
      if (!pending) {
        pending = true;
        window.requestAnimationFrame(follow);
      }
    }

    // A mouse presses the keys it passes over; a finger presses the keys
    // under it while it is down, and they come back up once it lifts.
    var lift;
    hero.addEventListener("pointermove", function (event) {
      window.clearTimeout(lift);
      moved(event.clientX, event.clientY);
    });
    hero.addEventListener("pointerdown", function (event) {
      window.clearTimeout(lift);
      moved(event.clientX, event.clientY);
    });
    function lifted(event) {
      if (event.pointerType === "mouse") {
        return;
      }
      window.clearTimeout(lift);
      lift = window.setTimeout(function () {
        moved(-1e4, -1e4);
      }, 300);
    }
    hero.addEventListener("pointerup", lifted);
    hero.addEventListener("pointercancel", lifted);
    hero.addEventListener("pointerleave", function (event) {
      if (event.pointerType === "mouse") {
        moved(-1e4, -1e4);
      }
    });

    // Scrolling. The page's acceleration, read from frame to frame, pushes
    // every key the other way, as if it were loose on a moving sheet; the
    // spring then brings it back. The loop runs from the first scroll
    // until every key is at rest again and the page has stopped moving.
    var swinging = false;
    var knownTop = window.scrollY;
    var lastTop = 0;
    var lastTick = 0;
    var lastScroll = 0;
    var pageSpeed = 0;

    function rest(key) {
      key.off = 0;
      key.speed = 0;
      key.el.style.setProperty("--dy", "0px");
      key.el.style.setProperty("--dr", "0deg");
    }

    function swing(now) {
      var dt = Math.max(0.008, Math.min(0.032, (now - lastTick) / 1000));
      lastTick = now;
      var top = window.scrollY;
      var speed = (top - lastTop) / dt;
      lastTop = top;
      var push = Math.max(-9000, Math.min(9000, ((speed - pageSpeed) / dt) * 0.11));
      pageSpeed = speed;
      var resting = now - lastScroll > 120;
      keys.forEach(function (key) {
        var force = push * key.weight - key.spring * key.off - key.give * key.speed;
        key.speed += force * dt;
        key.off = Math.max(-30, Math.min(30, key.off + key.speed * dt));
        if (Math.abs(key.off) > 0.3 || Math.abs(key.speed) > 6) {
          resting = false;
        }
        key.el.style.setProperty("--dy", key.off.toFixed(2) + "px");
        key.el.style.setProperty(
          "--dr",
          Math.max(-10, Math.min(10, key.speed * 0.03 * key.tilt)).toFixed(2) + "deg"
        );
      });
      if (resting || !seen) {
        swinging = false;
        keys.forEach(rest);
        return;
      }
      window.requestAnimationFrame(swing);
    }

    window.addEventListener(
      "scroll",
      function () {
        lastScroll = performance.now();
        if (!swinging && seen) {
          swinging = true;
          // The first frame measures from where the page was before this
          // scroll, so even a single jump gives the keys a push.
          lastTop = knownTop;
          lastTick = lastScroll;
          pageSpeed = 0;
          window.requestAnimationFrame(swing);
        }
        knownTop = window.scrollY;
      },
      { passive: true }
    );

    function press(key) {
      key.classList.add("is-tap");
      window.setTimeout(function () {
        key.classList.remove("is-tap");
      }, 170);
    }

    var seen = true;
    if (observes) {
      new IntersectionObserver(function (entries) {
        seen = entries[0].isIntersecting;
      }).observe(hero);
    }

    // Keys are pressed on their own only once the headline is typed.
    var quiet = true;

    function twinkle() {
      if (seen && !document.hidden && !quiet && keys.length) {
        press(pick(keys).el);
      }
      window.setTimeout(twinkle, 1500 + Math.random() * 2000);
    }

    var timer;
    window.addEventListener("resize", function () {
      window.clearTimeout(timer);
      timer = window.setTimeout(layout, 150);
    });
    if (document.fonts && document.fonts.ready) {
      document.fonts.ready.then(layout);
    }
    layout();
    twinkle();

    return {
      tap: function (label) {
        var options = byLabel[label.toUpperCase()];
        if (options) {
          press(pick(options));
        }
      },
      idle: function () {
        quiet = false;
      }
    };
  }

  // ---- The headline -----------------------------------------------------

  // The headline is typed out one letter at a time under a caret, which
  // blinks on for a moment at the end. Assistive technology reads the
  // whole sentence from a hidden copy; the letters it is typed from are
  // in the page from the start, only hidden, so the lines never move. A
  // page opened in a background tab waits to be looked at before typing.

  function whenSeen(start) {
    if (!document.hidden) {
      start();
      return;
    }
    document.addEventListener("visibilitychange", function once() {
      if (!document.hidden) {
        document.removeEventListener("visibilitychange", once);
        start();
      }
    });
  }

  function typeOut(title, onLetter, done) {
    var text = title.textContent.replace(/\s+/g, " ").trim();
    var spoken = document.createElement("span");
    spoken.className = "visually-hidden";
    spoken.textContent = text;
    var shown = document.createElement("span");
    shown.setAttribute("aria-hidden", "true");
    var letters = [];
    text.split(" ").forEach(function (word, index) {
      if (index) {
        shown.appendChild(document.createTextNode(" "));
      }
      var span = document.createElement("span");
      span.className = "word";
      word.split("").forEach(function (letter) {
        var ch = document.createElement("span");
        ch.className = "ch";
        ch.textContent = letter;
        span.appendChild(ch);
        letters.push(ch);
      });
      shown.appendChild(span);
    });
    title.classList.add("is-typing");
    title.textContent = "";
    title.appendChild(spoken);
    title.appendChild(shown);

    var at = 0;
    function next() {
      if (at) {
        letters[at - 1].classList.remove("is-caret");
      }
      if (at === letters.length) {
        title.classList.remove("is-typing");
        done();
        return;
      }
      var ch = letters[at];
      ch.classList.add("is-on", "is-caret");
      onLetter(ch.textContent);
      at += 1;
      var wait = 45 + Math.random() * 45;
      if (ch.textContent === ",") {
        wait += 320;
      }
      if (at === letters.length) {
        wait = 1500;
      }
      window.setTimeout(next, wait);
    }
    window.setTimeout(next, 420);
  }

  var hero = document.querySelector(".hero");
  var title = hero && hero.querySelector("h1");
  var shot = hero && hero.querySelector(".shot");
  var field = hero && shot && !still ? keyField(hero, shot) : null;
  if (title && !still) {
    whenSeen(function () {
      typeOut(
        title,
        function (letter) {
          if (field && /[a-z]/i.test(letter)) {
            field.tap(letter);
          }
        },
        function () {
          if (field) {
            field.idle();
          }
        }
      );
    });
  } else if (field) {
    field.idle();
  }

  // ---- Scrolling in -----------------------------------------------------

  // Each part marked data-reveal comes in once it scrolls into view, and
  // a tile's illustration loops only while the tile is on screen.

  var parts = Array.prototype.slice.call(document.querySelectorAll("[data-reveal]"));
  var cards = Array.prototype.slice.call(document.querySelectorAll(".card"));
  if (still || !observes) {
    parts.forEach(function (part) {
      part.classList.add("is-in");
    });
  } else {
    var revealer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            entry.target.classList.add("is-in");
            revealer.unobserve(entry.target);
          }
        });
      },
      { rootMargin: "0px 0px -8% 0px", threshold: 0.1 }
    );
    parts.forEach(function (part) {
      revealer.observe(part);
    });
    var player = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          entry.target.classList.toggle("is-playing", entry.isIntersecting);
        });
      },
      { threshold: 0.3 }
    );
    cards.forEach(function (card) {
      player.observe(card);
    });
  }

  // ---- The recording ----------------------------------------------------

  var video = document.querySelector(".shot video");
  if (video) {
    var shotFigure = video.closest(".shot");
    var controls = document.createElement("div");
    controls.className = "shot-controls";

    var toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "shot-toggle";
    var label = function () {
      toggle.textContent = video.paused ? "Play" : "Pause";
    };
    toggle.addEventListener("click", function () {
      if (video.paused) {
        video.play();
      } else {
        video.pause();
      }
    });
    video.addEventListener("play", label);
    video.addEventListener("pause", label);
    if (still) {
      video.removeAttribute("autoplay");
      video.pause();
    }
    label();

    var size = document.createElement("button");
    size.type = "button";
    size.className = "shot-toggle";
    var expanded = function () {
      return shotFigure.classList.contains("is-expanded");
    };
    // Once the recording is wide, scroll just enough to bring all of it
    // into view (or its top, in a window too short even for the column).
    var settle = function () {
      if (expanded()) {
        shotFigure.scrollIntoView({ block: "nearest" });
      }
    };
    var resize = function (wide) {
      shotFigure.classList.toggle("is-expanded", wide);
      size.textContent = wide ? "Shrink" : "Expand";
      if (wide && still) {
        settle();
      }
    };
    size.addEventListener("click", function () {
      resize(!expanded());
    });
    video.addEventListener("click", function () {
      resize(!expanded());
    });
    shotFigure.addEventListener("transitionend", function (event) {
      if (event.target === shotFigure && event.propertyName === "width") {
        settle();
      }
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && expanded()) {
        resize(false);
      }
    });
    resize(false);

    controls.appendChild(toggle);
    controls.appendChild(size);
    shotFigure.appendChild(controls);
  }

  // ---- Installing -------------------------------------------------------

  var installer = document.querySelector("[data-installer]");
  if (!installer) {
    return;
  }

  var STORAGE_KEY = "keyloom-install";
  var distroBar = installer.querySelector("[data-distro-tabs]");
  var versionBar = installer.querySelector("[data-version-tabs]");
  var distros = Array.prototype.slice.call(installer.querySelectorAll(".distro"));

  function versionsOf(distro) {
    return Array.prototype.slice.call(distro.querySelectorAll(".version"));
  }

  function remembered() {
    try {
      return JSON.parse(window.localStorage.getItem(STORAGE_KEY) || "null") || {};
    } catch (error) {
      return {};
    }
  }

  var saved = remembered();
  var activeDistro = null;
  var activeVersion = {};

  distros.forEach(function (distro) {
    var id = distro.dataset.distro;
    var versions = versionsOf(distro);
    var chosen = null;
    versions.forEach(function (version) {
      if (saved.versions && version.dataset.version === saved.versions[id]) {
        chosen = version;
      }
    });
    if (!chosen) {
      versions.forEach(function (version) {
        if (!chosen && version.classList.contains("is-active")) {
          chosen = version;
        }
      });
    }
    activeVersion[id] = chosen || versions[0];
    if (id === saved.distro) {
      activeDistro = distro;
    }
  });
  if (!activeDistro) {
    distros.forEach(function (distro) {
      if (!activeDistro && distro.classList.contains("is-active")) {
        activeDistro = distro;
      }
    });
  }
  activeDistro = activeDistro || distros[0];

  function remember() {
    var versions = {};
    Object.keys(activeVersion).forEach(function (id) {
      versions[id] = activeVersion[id].dataset.version;
    });
    try {
      window.localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ distro: activeDistro.dataset.distro, versions: versions })
      );
    } catch (error) {
      // Private windows and blocked storage: the choice just is not kept.
    }
  }

  function button(text, className, pressed) {
    var element = document.createElement("button");
    element.type = "button";
    element.className = className;
    element.textContent = text;
    element.setAttribute("aria-pressed", pressed ? "true" : "false");
    return element;
  }

  function render() {
    distros.forEach(function (distro) {
      var on = distro === activeDistro;
      distro.classList.toggle("is-active", on);
      distro.hidden = !on;
      versionsOf(distro).forEach(function (version) {
        var onVersion = version === activeVersion[distro.dataset.distro];
        version.classList.toggle("is-active", onVersion);
        version.hidden = !onVersion;
      });
    });

    Array.prototype.forEach.call(distroBar.children, function (tab) {
      tab.setAttribute("aria-pressed", tab.dataset.for === activeDistro.dataset.distro ? "true" : "false");
    });

    versionBar.textContent = "";
    versionsOf(activeDistro).forEach(function (version) {
      var pill = button(
        version.dataset.name,
        "pill",
        version === activeVersion[activeDistro.dataset.distro]
      );
      pill.dataset.for = version.dataset.version;
      pill.addEventListener("click", function () {
        activeVersion[activeDistro.dataset.distro] = version;
        remember();
        render();
      });
      versionBar.appendChild(pill);
    });
  }

  distros.forEach(function (distro) {
    var tab = button(distro.dataset.name, "tab", distro === activeDistro);
    tab.dataset.for = distro.dataset.distro;
    tab.addEventListener("click", function () {
      activeDistro = distro;
      remember();
      render();
    });
    distroBar.appendChild(tab);
  });
  render();

  Array.prototype.forEach.call(installer.querySelectorAll("[data-copy]"), function (copy) {
    copy.addEventListener("click", function () {
      var code = copy.closest(".terminal").querySelector("code");
      var text = code.textContent.trim();
      var select = function () {
        var range = document.createRange();
        range.selectNodeContents(code);
        var selection = window.getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
      };
      if (!navigator.clipboard || !navigator.clipboard.writeText) {
        select();
        return;
      }
      navigator.clipboard.writeText(text).then(
        function () {
          var text = copy.textContent;
          copy.textContent = "Copied";
          copy.classList.add("is-done");
          window.setTimeout(function () {
            copy.textContent = text;
            copy.classList.remove("is-done");
          }, 1800);
        },
        select
      );
    });
  });
})();
