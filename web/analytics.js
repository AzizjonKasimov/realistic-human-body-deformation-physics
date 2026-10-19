// Counts visits to the live page with GoatCounter (https://www.goatcounter.com),
// which sets no cookies and keeps nothing on the visitor's device, and a few
// events that show what visitors did: pressing Start, the simulation loading
// or failing to, the first blow with each tool, how long they played, whether
// the frame rate held, which links they followed, and whether they installed
// the game as an app or opened it as one.
//
// Only the published site sends anything. Local builds (serve_web.ps1, the
// parity check, the browser pane), copies published elsewhere, and ?selftest
// runs count nothing. Every event, sent or not, also lands in
// window.__analytics, so a local check sees what the live site would send.
//
// The Fight page (fight/, the game built on the engine) loads this script
// too, with data-events="fight-" on its script tag, so its events count apart
// from the sandbox's: fight-start, fight-loaded, fight-played-1min, and the
// game's own, such as fight-punch.
(function () {
  "use strict";

  // What this page's event names begin with: nothing for the sandbox.
  const EVENT_PREFIX = (document.currentScript && document.currentScript.dataset.events) || "";

  // The GoatCounter site the counts go to; its dashboard is
  // https://<code>.goatcounter.com. Left empty, nothing is sent.
  const SITE_CODE = "realistic-physics";
  const LIVE_HOST = "azizjonkasimov.github.io";
  // A fixed version of GoatCounter's script with its hash, so browsers refuse
  // it if it ever changes (https://www.goatcounter.com/help/countjs-versions).
  const COUNT_JS = "https://gc.zgo.at/count.v5.js";
  const COUNT_JS_INTEGRITY = "sha384-atnOLvQb9t+jTSipvd75X2yginT4PjVbqDdlJAmxMm+wYElFmeR6EmLP5bYeoRVQ";

  // Minutes of play that each count once.
  const PLAY_MINUTES = [1, 5, 15];
  // A visitor who has not moved the pointer, touched, or pressed a key for
  // this long is away, not playing.
  const IDLE_MS = 60000;
  // Seconds of play whose frame rates decide the frame-rate event.
  const FPS_SECONDS = 20;

  if (new URLSearchParams(location.search).has("selftest")) {
    return;
  }

  const gate = document.getElementById("gate");
  const startButton = document.getElementById("start");
  const status = document.getElementById("status");

  const events = [];
  window.__analytics = events;

  // Calls to GoatCounter wait here until its script loads; null once it has.
  // Sending stops for good if the script cannot load (blocked, or offline).
  let waiting = [];
  let sending = Boolean(SITE_CODE) && location.hostname === LIVE_HOST;

  function send(vars) {
    if (!sending) {
      return;
    }
    if (waiting) {
      waiting.push(vars);
    } else {
      window.goatcounter.count(vars);
    }
  }

  function count(name, title) {
    name = EVENT_PREFIX + name;
    title = (EVENT_PREFIX ? "Fight: " : "") + title;
    events.push({ name: name, title: title, at: Math.round(performance.now()) });
    send({ path: name, title: title, event: true });
  }

  if (sending) {
    window.goatcounter = { no_onload: true };
    const script = document.createElement("script");
    script.async = true;
    script.src = COUNT_JS;
    script.integrity = COUNT_JS_INTEGRITY;
    script.crossOrigin = "anonymous";
    script.dataset.goatcounter = "https://" + SITE_CODE + ".goatcounter.com/count";
    script.addEventListener("load", function () {
      const queued = waiting;
      waiting = null;
      queued.forEach(function (vars) {
        window.goatcounter.count(vars);
      });
    });
    script.addEventListener("error", function () {
      sending = false;
      waiting = null;
    });
    document.head.appendChild(script);
  }

  // The visit, under the page's path without the query, so ?stats or a
  // link's tracking tags do not split it. GoatCounter still reads campaign
  // tags such as ?ref=linkedin from the query it is sent.
  events.push({ name: "visit", title: document.title, at: Math.round(performance.now()) });
  send({ path: location.pathname, title: document.title });

  // Visits that open the game installed as an app (from the home screen, the
  // computer's apps, or the Dock), and installs, from the start screen's
  // button or the browser's own menu. Safari tells the page of no installs.
  const openedAsApp = navigator.standalone === true || ["standalone", "fullscreen", "minimal-ui"].some(function (mode) {
    return window.matchMedia("(display-mode: " + mode + ")").matches;
  });
  if (openedAsApp) {
    count("opened-as-app", "Opened as an installed app");
  }
  window.addEventListener("appinstalled", function () {
    count("installed", "Installed as an app");
  });

  // Start, then whether the simulation loaded. The start button doubles as
  // the way back to the simulation later, so only its first press counts.
  let pressedStart = false;
  let loaded = false;
  startButton.addEventListener("click", function () {
    if (pressedStart) {
      return;
    }
    pressedStart = true;
    count("start", "Pressed Start simulation");
    const poll = setInterval(function () {
      if (window.wasm_exports) {
        clearInterval(poll);
        loaded = true;
        count("loaded", "Simulation loaded");
      } else if (!status.hidden && status.classList.contains("error")) {
        // Without WebGL the page keeps its start screen up; a failed
        // download or startup has already hidden it.
        clearInterval(poll);
        if (gate.hidden) {
          count("load-failed", "Simulation failed to load");
        } else {
          count("no-webgl", "Could not start: WebGL off or unsupported");
        }
      }
    }, 250);
  });

  // The first blow with each tool, reported by the app through the page's
  // rp_tool_used import, and for checks, the play time counted so far.
  const toolsUsed = {};
  let struck = false;
  let playedMs = 0;
  // Things the game does once, which it names itself (fight-punch, for the
  // game's first punch).
  const counted = {};
  window.rpAnalytics = {
    playedSeconds: function () {
      return playedMs / 1000;
    },
    event: function (name) {
      if (counted[name]) {
        return;
      }
      counted[name] = true;
      count(name, name.replace(/-/g, " "));
    },
    toolUsed: function (tool) {
      if (toolsUsed[tool]) {
        return;
      }
      toolsUsed[tool] = true;
      if (!struck) {
        struck = true;
        count("hit", "Struck the body");
      }
      count("tool-" + tool, "Struck with the " + tool);
    }
  };

  // Play time counts while the simulation is on screen (not under the start
  // or about card), the tab is in front, and the visitor is not away.
  let lastInput = -Infinity;
  [
    "pointerdown", "pointermove", "mousedown", "mousemove", "touchstart", "touchmove", "keydown", "wheel"
  ].forEach(function (type) {
    window.addEventListener(type, function () {
      lastInput = performance.now();
    }, { capture: true, passive: true });
  });

  // gl.js asks for every frame through requestAnimationFrame, so wrapping it
  // counts frames without asking for any of its own. A simulation that
  // crashes throws out of its frame and asks for no more.
  let frames = 0;
  const nativeRaf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = function (callback) {
    return nativeRaf(function (time) {
      frames += 1;
      callback(time);
    });
  };
  let errorSeen = false;
  window.addEventListener("error", function () {
    errorSeen = true;
  });

  let minutesCounted = 0;
  const frameRates = [];
  let crashCounted = false;
  let lastTick = performance.now();
  setInterval(function () {
    const now = performance.now();
    const elapsed = now - lastTick;
    const framesThisTick = frames;
    lastTick = now;
    frames = 0;
    const visible = document.visibilityState === "visible";
    if (loaded && errorSeen && visible && framesThisTick === 0 && !crashCounted) {
      crashCounted = true;
      count("crashed", "Simulation stopped with an error");
    }
    if (!loaded || !gate.hidden || !visible || now - lastInput >= IDLE_MS) {
      return;
    }
    // A late tick (a busy page) counts at most two seconds.
    playedMs += Math.min(elapsed, 2000);
    while (minutesCounted < PLAY_MINUTES.length && playedMs >= PLAY_MINUTES[minutesCounted] * 60000) {
      const minutes = PLAY_MINUTES[minutesCounted];
      count("played-" + minutes + "min", "Played " + minutes + (minutes === 1 ? " minute" : " minutes"));
      minutesCounted += 1;
    }
    if (frameRates.length < FPS_SECONDS && elapsed > 500 && elapsed < 2000) {
      frameRates.push((framesThisTick * 1000) / elapsed);
      if (frameRates.length === FPS_SECONDS) {
        countFrameRate();
      }
    }
  }, 1000);

  // How smoothly the typical second of play ran, on a touch screen (phones,
  // tablets) or with a mouse or touchpad (computers).
  function countFrameRate() {
    const sorted = frameRates.slice().sort(function (a, b) { return a - b; });
    const median = sorted[Math.floor(sorted.length / 2)];
    const touch = window.matchMedia("(pointer: coarse)").matches;
    const band = median >= 50 ? "smooth" : median >= 30 ? "uneven" : "choppy";
    const rate = median >= 50 ? "50 fps or more" : median >= 30 ? "30-49 fps" : "under 30 fps";
    count(
      "fps-" + band + "-" + (touch ? "touch" : "mouse"),
      "Frame rate " + rate + (touch ? " on a touch screen" : " with a mouse or touchpad")
    );
  }

  // Links followed from the page, named after their text ("link-source-code").
  function linkFollowed(event) {
    if (event.type === "auxclick" && event.button !== 1) {
      return;
    }
    const link = event.target.closest && event.target.closest("a[href]");
    if (!link) {
      return;
    }
    const text = link.textContent.trim();
    const slug = text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
    count("link-" + (slug || "unnamed"), "Followed the " + (text || link.href) + " link");
  }
  document.addEventListener("click", linkFollowed, true);
  document.addEventListener("auxclick", linkFollowed, true);
})();
