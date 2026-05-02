"use strict";

(function initSandbox(RP) {
  const canvas = document.getElementById("sandbox");
  const ctx = canvas.getContext("2d");
  const startScreen = document.getElementById("start-screen");
  const startButton = document.getElementById("start-button");
  const resetButton = document.getElementById("reset-button");
  const quitButton = document.getElementById("quit-button");
  const impactButton = document.getElementById("impact-button");
  const debugButton = document.getElementById("debug-button");
  const qaForearmButton = document.getElementById("qa-forearm-button");
  const qaShinButton = document.getElementById("qa-shin-button");
  const qaRestButton = document.getElementById("qa-rest-button");
  const stats = document.getElementById("stats");
  const qaReport = document.getElementById("qa-report");

  const pointer = RP.Input.createPointer(canvas);
  const renderer = new RP.Renderer.Renderer(canvas, ctx);
  const qa = new RP.QA.Harness({
    reset: buildWorld,
    setRunning: (value) => {
      running = value;
      accumulator = 0;
    },
    world: () => world,
    pointer: () => pointer,
    draw: () => {
      updateStats();
      renderer.draw(world, pointer);
    },
    showReport: (text) => {
      qaReport.textContent = text;
    }
  });

  let world = null;
  let running = false;
  let lastTime = 0;
  let accumulator = 0;
  let dpr = 1;
  const impactModes = [
    { label: "Impact 1x", power: 1 },
    { label: "Impact 2x", power: 2 },
    { label: "Impact 4x", power: 4 }
  ];
  let impactModeIndex = 1;

  function resize() {
    dpr = Math.max(1, Math.min(window.devicePixelRatio || 1, 2));
    canvas.width = Math.floor(window.innerWidth * dpr);
    canvas.height = Math.floor(window.innerHeight * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    buildWorld();
  }

  function buildWorld() {
    world = RP.BodyFactory.createLayeredBody(window.innerWidth, window.innerHeight);
    window.__debugWorld = world;
    updateStats();
  }

  function updateStats() {
    if (!world) {
      stats.textContent = "0 skin tears, 0 muscle tears";
      return;
    }
    stats.textContent = `${world.stats.brokenSkin} skin, ${world.stats.brokenMuscle} muscle, ${world.stats.boneFractures} bone, ${world.stats.brokenAttachments} detach`;
  }

  function applyImpactMode() {
    const mode = impactModes[impactModeIndex];
    pointer.power = mode.power;
    impactButton.textContent = mode.label;
  }

  function animate(time) {
    if (!lastTime) lastTime = time;
    const frameDt = Math.min(0.05, (time - lastTime) / 1000);
    lastTime = time;

    if (running && world) {
      accumulator += frameDt;
      while (accumulator >= RP.Materials.config.fixedDt) {
        world.step(RP.Materials.config.fixedDt, pointer);
        accumulator -= RP.Materials.config.fixedDt;
      }
      updateStats();
    }

    renderer.draw(world, pointer);
    requestAnimationFrame(animate);
  }

  window.addEventListener("resize", resize);

  startButton.addEventListener("click", () => {
    running = true;
    startScreen.classList.add("hidden");
  });

  resetButton.addEventListener("click", () => {
    buildWorld();
    qaReport.textContent = "";
  });

  quitButton.addEventListener("click", () => {
    running = false;
    accumulator = 0;
    startScreen.classList.remove("hidden");
  });

  debugButton.addEventListener("click", () => {
    renderer.debug.enabled = !renderer.debug.enabled;
    debugButton.classList.toggle("active", renderer.debug.enabled);
  });

  impactButton.addEventListener("click", () => {
    impactModeIndex = (impactModeIndex + 1) % impactModes.length;
    applyImpactMode();
  });

  qaForearmButton.addEventListener("click", () => {
    startScreen.classList.add("hidden");
    renderer.debug.enabled = true;
    debugButton.classList.add("active");
    qa.run("forearm");
  });

  qaShinButton.addEventListener("click", () => {
    startScreen.classList.add("hidden");
    renderer.debug.enabled = true;
    debugButton.classList.add("active");
    qa.run("shin");
  });

  qaRestButton.addEventListener("click", () => {
    startScreen.classList.add("hidden");
    renderer.debug.enabled = true;
    debugButton.classList.add("active");
    qa.run("rest");
  });

  resize();
  applyImpactMode();
  requestAnimationFrame(animate);
})(window.RP);
