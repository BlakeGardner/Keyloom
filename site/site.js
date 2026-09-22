// Keyloom website: the hero recording's pause button, and the install
// section's distribution and version tabs with their copy buttons. Without
// scripting, the recording still loops and every install block shows in
// order.
(function () {
  "use strict";

  var video = document.querySelector(".shot video");
  if (video) {
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
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      video.removeAttribute("autoplay");
      video.pause();
    }
    label();
    video.parentNode.appendChild(toggle);
  }

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
    var pick = null;
    versions.forEach(function (version) {
      if (saved.versions && version.dataset.version === saved.versions[id]) {
        pick = version;
      }
    });
    if (!pick) {
      versions.forEach(function (version) {
        if (!pick && version.classList.contains("is-active")) {
          pick = version;
        }
      });
    }
    activeVersion[id] = pick || versions[0];
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

  function button(label, className, pressed) {
    var element = document.createElement("button");
    element.type = "button";
    element.className = className;
    element.textContent = label;
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
          var label = copy.textContent;
          copy.textContent = "Copied";
          copy.classList.add("is-done");
          window.setTimeout(function () {
            copy.textContent = label;
            copy.classList.remove("is-done");
          }, 1800);
        },
        select
      );
    });
  });
})();
