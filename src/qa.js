"use strict";

(function initQA(RP) {
  const fixedDt = 1 / 60;

  const scenarios = {
    rest: {
      name: "Rest stability",
      steps: 120,
      targetBone: "right forearm",
      strike: null
    },
    forearm: {
      name: "Right forearm fracture",
      steps: 240,
      targetBone: "right forearm",
      strike: { axis: "x", reach: 150, passes: 6 }
    },
    shin: {
      name: "Right shin fracture",
      steps: 240,
      targetBone: "right shin",
      strike: { axis: "x", reach: 160, passes: 6 }
    }
  };

  class Harness {
    constructor(api) {
      this.api = api;
      this.lastReport = null;
    }

    run(name) {
      const scenario = scenarios[name];
      if (!scenario) throw new Error(`Unknown QA scenario: ${name}`);

      this.api.reset();
      this.api.setRunning(false);
      const world = this.api.world();
      const pointer = this.api.pointer();
      const initial = snapshot(world, scenario.targetBone);
      const path = buildPath(world, scenario);
      let pathIndex = 0;
      let maxTargetHealth = 0;
      let maxTargetImpulse = 0;
      let totalStepMs = 0;
      let maxStepMs = 0;

      pointer.active = path.length > 0;
      pointer.down = false;
      pointer.vx = 0;
      pointer.vy = 0;

      for (let frame = 0; frame < scenario.steps; frame++) {
        while (pathIndex < path.length && path[pathIndex].frame <= frame) {
          applyPathPoint(pointer, path[pathIndex]);
          pathIndex++;
        }

        maxTargetHealth = Math.max(maxTargetHealth, targetStressRatio(world, scenario.targetBone));
        const beforeStep = performance.now();
        world.step(fixedDt, pointer);
        const stepMs = performance.now() - beforeStep;
        totalStepMs += stepMs;
        maxStepMs = Math.max(maxStepMs, stepMs);
        maxTargetImpulse = Math.max(maxTargetImpulse, maxImpulseFor(world, scenario.targetBone));
      }

      pointer.down = false;
      const finalState = snapshot(world, scenario.targetBone);
      const report = buildReport(scenario, initial, finalState, maxTargetHealth, maxTargetImpulse, totalStepMs / scenario.steps, maxStepMs);
      this.lastReport = report;
      this.api.showReport(formatReport(report));
      this.api.draw();
      return report;
    }
  }

  function applyPathPoint(pointer, point) {
    const oldX = pointer.x;
    const oldY = pointer.y;
    pointer.px = oldX;
    pointer.py = oldY;
    pointer.x = point.x;
    pointer.y = point.y;
    pointer.vx = point.x - oldX;
    pointer.vy = point.y - oldY;
    pointer.down = point.down;
    pointer.active = true;
  }

  function buildPath(world, scenario) {
    if (!scenario.strike) return [];
    const target = targetState(world, scenario.targetBone);
    if (!target) return [];

    const reach = scenario.strike.reach;
    const framesPerPass = 12;
    const path = [];
    addSegment(path, 0, 6, target.x + reach, target.y, target.x + reach * 0.62, target.y, false);
    let frame = 6;
    for (let pass = 0; pass < scenario.strike.passes; pass++) {
      addSegment(path, frame, frame + framesPerPass, target.x + reach * 0.62, target.y, target.x - reach * 0.62, target.y, true);
      frame += framesPerPass;
      addSegment(path, frame, frame + framesPerPass, target.x - reach * 0.62, target.y, target.x + reach * 0.62, target.y, true);
      frame += framesPerPass;
    }
    addSegment(path, frame, frame + 6, target.x + reach * 0.62, target.y, target.x + reach * 0.75, target.y, false);
    return path;
  }

  function addSegment(path, startFrame, endFrame, x1, y1, x2, y2, down) {
    const count = Math.max(1, endFrame - startFrame);
    for (let i = 0; i <= count; i++) {
      const t = i / count;
      path.push({
        frame: startFrame + i,
        x: x1 + (x2 - x1) * t,
        y: y1 + (y2 - y1) * t,
        down
      });
    }
  }

  function snapshot(world, targetBone) {
    const activeBones = world.bones.filter((bone) => bone.active);
    const target = targetState(world, targetBone);
    const fragments = world.bones.filter((bone) => bone.active && bone.parentName === targetBone && bone.dynamic);
    return {
      stats: { ...world.stats },
      activeBoneCount: activeBones.length,
      target: target ? boneState(target) : null,
      fragments: fragments.map(boneState),
      allDynamicFragments: activeBones.filter((bone) => bone.dynamic).map(boneState)
    };
  }

  function boneState(bone) {
    return {
      name: bone.name,
      x: bone.x,
      y: bone.y,
      angle: bone.angle,
      health: bone.health,
      strength: bone.strength,
      dynamic: bone.dynamic
    };
  }

  function buildReport(scenario, initial, finalState, maxTargetHealth, maxTargetImpulse, avgStepMs, maxStepMs) {
    const targetFragments = finalState.fragments;
    const fragmentMovement = targetFragments.reduce((max, fragment) => {
      const start = initial.target || fragment;
      const movement = Math.hypot(fragment.x - start.x, fragment.y - start.y);
      return Math.max(max, movement);
    }, 0);

    const assertions = scenario.name === "Rest stability"
      ? [
          pass("no fractures at rest", finalState.stats.boneFractures === 0, `fractures=${finalState.stats.boneFractures}`),
          pass("bone count stable", finalState.activeBoneCount === initial.activeBoneCount, `${initial.activeBoneCount}->${finalState.activeBoneCount}`),
          pass("no tissue damage at rest", finalState.stats.brokenSkin === 0 && finalState.stats.brokenMuscle === 0, `skin=${finalState.stats.brokenSkin}, muscle=${finalState.stats.brokenMuscle}`)
        ]
      : [
          pass("target bone received impulse", maxTargetImpulse > 40, `maxImpulse=${Math.round(maxTargetImpulse)}`),
          pass("target joint stress rose", maxTargetHealth > 0.45, `maxStress=${maxTargetHealth.toFixed(2)}`),
          pass("target bone fractured", targetFragments.length >= 2, `fragments=${targetFragments.length}`),
          pass("fracture stays local", reportFractureCount(finalState, scenario.targetBone) <= 3, `targetFractures=${reportFractureCount(finalState, scenario.targetBone)}`),
          pass("fragments moved visibly", fragmentMovement > 18, `movement=${fragmentMovement.toFixed(1)}px`),
          pass("fragments remain inspectable", fragmentMovement < 700, `movement=${fragmentMovement.toFixed(1)}px`)
        ];

    return {
      name: scenario.name,
      targetBone: scenario.targetBone,
      maxTargetHealth,
      maxTargetImpulse,
      avgStepMs,
      maxStepMs,
      fragmentMovement,
      initial,
      finalState,
      assertions,
      ok: assertions.every((assertion) => assertion.ok)
    };
  }

  function pass(label, ok, detail) {
    return { label, ok, detail };
  }

  function formatReport(report) {
    const status = report.ok ? "PASS" : "FAIL";
    const lines = [
      `QA ${status}: ${report.name}`,
      `target: ${report.targetBone}`,
      `max stress: ${report.maxTargetHealth.toFixed(2)}`,
      `max impulse: ${Math.round(report.maxTargetImpulse)}`,
      `fragment movement: ${report.fragmentMovement.toFixed(1)}px`,
      `step time: avg ${report.avgStepMs.toFixed(2)}ms, max ${report.maxStepMs.toFixed(2)}ms`,
      ""
    ];

    for (const assertion of report.assertions) {
      lines.push(`${assertion.ok ? "PASS" : "FAIL"} ${assertion.label} (${assertion.detail})`);
    }

    lines.push("");
    lines.push(`stats: skin=${report.finalState.stats.brokenSkin}, muscle=${report.finalState.stats.brokenMuscle}, bone=${report.finalState.stats.boneFractures}, detach=${report.finalState.stats.brokenAttachments}`);
    return lines.join("\n");
  }

  function targetState(world, parentName) {
    const segments = world.bones.filter((bone) => bone.active && bone.parentName === parentName);
    if (segments.length === 0) return null;
    const sum = segments.reduce((acc, bone) => {
      acc.x += bone.x;
      acc.y += bone.y;
      acc.health = Math.max(acc.health, bone.health / Math.max(1, bone.strength));
      return acc;
    }, { x: 0, y: 0, health: 0 });
    return {
      name: parentName,
      x: sum.x / segments.length,
      y: sum.y / segments.length,
      angle: segments[0].angle,
      health: sum.health,
      strength: 1,
      dynamic: segments.some((bone) => bone.dynamic)
    };
  }

  function targetStressRatio(world, parentName) {
    let ratio = 0;
    for (const joint of world.boneJoints) {
      if (joint.parentName !== parentName || joint.broken) continue;
      ratio = Math.max(ratio, joint.damage / Math.max(1, joint.strength), joint.stress / Math.max(1, joint.strength));
    }
    return ratio;
  }

  function reportFractureCount(state, parentName) {
    return state.allDynamicFragments.filter((bone) => bone.name.startsWith(parentName)).length > 0
      ? Math.max(1, state.fragments.length - 1)
      : 0;
  }

  function maxImpulseFor(world, boneName) {
    let maxImpulse = 0;
    for (const contact of world.debug.contacts) {
      if (contact.bone === boneName) maxImpulse = Math.max(maxImpulse, contact.impulse);
    }
    return maxImpulse;
  }

  RP.QA = {
    Harness,
    scenarios
  };
})(window.RP);
