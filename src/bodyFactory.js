"use strict";

(function initBodyFactory(RP) {
  function createLayeredBody(width, height) {
    const world = new RP.Physics.World(RP.Materials);
    const config = RP.Materials.config;
    const skin = RP.Materials.skin;
    const muscle = RP.Materials.muscle;

    const bodyHeight = Math.min(height * 0.78, width * 1.12, 720);
    const bodyWidth = bodyHeight * 0.64;
    const originX = width * 0.52;
    const originY = height * 0.09;
    const cols = Math.floor(bodyWidth / config.pointSpacing);
    const rows = Math.floor(bodyHeight / config.pointSpacing);
    const skinGrid = new Map();
    const muscleGrid = new Map();

    for (let y = 0; y <= rows; y++) {
      for (let x = 0; x <= cols; x++) {
        const nx = (x / cols - 0.5) * 0.7;
        const ny = y / rows;
        if (!isInsideHumanoid(nx, ny)) continue;

        const skinX = originX + (x / cols - 0.5) * bodyWidth;
        const skinY = originY + ny * bodyHeight;
        const muscleX = originX + (x / cols - 0.5) * bodyWidth * 0.78;
        const muscleY = originY + ny * bodyHeight;
        const pinned = ny < 0.035;
        const key = `${x},${y}`;

        const skinIndex = world.addPoint(skinX, skinY, "skin", pinned);
        const muscleIndex = world.addPoint(muscleX, muscleY, "muscle", pinned);
        skinGrid.set(key, skinIndex);
        muscleGrid.set(key, muscleIndex);
        world.addAttachment(skinIndex, muscleIndex);
      }
    }

    const addSkinSpring = (a, b, stiffness, tearStretch = skin.tearStretch) => {
      if (a === undefined || b === undefined) return;
      world.addSkinSpring(a, b, {
        stiffness,
        tearStretch,
        tearImpulse: skin.tearImpulse
      });
    };

    const addMuscleSpring = (a, b, stiffness, fiber = false, tearStretch = muscle.tearStretch) => {
      if (a === undefined || b === undefined) return;
      world.addMuscleSpring(a, b, {
        stiffness,
        fiber,
        tearStretch,
        tearImpulse: muscle.tearImpulse
      });
    };

    for (let y = 0; y <= rows; y++) {
      for (let x = 0; x <= cols; x++) {
        const skinPoint = skinGrid.get(`${x},${y}`);
        const musclePoint = muscleGrid.get(`${x},${y}`);
        if (skinPoint === undefined || musclePoint === undefined) continue;

        addSkinSpring(skinPoint, skinGrid.get(`${x + 1},${y}`), skin.structuralStiffness);
        addSkinSpring(skinPoint, skinGrid.get(`${x},${y + 1}`), skin.structuralStiffness);
        addSkinSpring(skinPoint, skinGrid.get(`${x + 1},${y + 1}`), skin.shearStiffness, skin.tearStretch * 1.08);
        addSkinSpring(skinPoint, skinGrid.get(`${x - 1},${y + 1}`), skin.shearStiffness, skin.tearStretch * 1.08);

        addMuscleSpring(musclePoint, muscleGrid.get(`${x},${y + 1}`), muscle.fiberStiffness, true);
        addMuscleSpring(musclePoint, muscleGrid.get(`${x},${y + 2}`), muscle.fiberStiffness * 0.42, true, muscle.tearStretch * 1.05);
        addMuscleSpring(musclePoint, muscleGrid.get(`${x + 1},${y}`), muscle.crossStiffness);
        addMuscleSpring(musclePoint, muscleGrid.get(`${x + 1},${y + 1}`), muscle.shearStiffness, false, muscle.tearStretch * 1.12);
        addMuscleSpring(musclePoint, muscleGrid.get(`${x - 1},${y + 1}`), muscle.shearStiffness, false, muscle.tearStretch * 1.12);
      }
    }

    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < cols; x++) {
        addCellTriangles(world, skinGrid, x, y, "skin", skin.areaStiffness);
        addCellTriangles(world, muscleGrid, x, y, "muscle", muscle.areaStiffness);
      }
    }

    addBones(world, originX, originY, bodyHeight, bodyWidth);

    return world;
  }

  function addBones(world, originX, originY, bodyHeight, bodyWidth) {
    const p = (nx, ny) => ({
      x: originX + nx * bodyWidth,
      y: originY + ny * bodyHeight
    });
    const add = (name, ax, ay, bx, by, radius, strength) => {
      const a = p(ax, ay);
      const b = p(bx, by);
      world.addBone(name, a.x, a.y, b.x, b.y, radius, strength);
    };

    add("skull", 0, 0.075, 0, 0.155, 14, 2600);
    add("spine", 0, 0.23, 0, 0.66, 10, 2300);
    add("left humerus", -0.19, 0.31, -0.235, 0.47, 7, 980);
    add("left forearm", -0.235, 0.47, -0.25, 0.62, 6, 760);
    add("right humerus", 0.19, 0.31, 0.235, 0.47, 7, 980);
    add("right forearm", 0.235, 0.47, 0.25, 0.62, 6, 760);
    add("left femur", -0.07, 0.67, -0.08, 0.82, 8, 1180);
    add("left shin", -0.08, 0.82, -0.085, 0.97, 7, 600);
    add("right femur", 0.07, 0.67, 0.08, 0.82, 8, 1180);
    add("right shin", 0.08, 0.82, 0.085, 0.97, 7, 600);
  }

  function addCellTriangles(world, grid, x, y, layer, areaStiffness) {
    const a = grid.get(`${x},${y}`);
    const b = grid.get(`${x + 1},${y}`);
    const c = grid.get(`${x},${y + 1}`);
    const d = grid.get(`${x + 1},${y + 1}`);
    if (a !== undefined && b !== undefined && c !== undefined) {
      world.addTriangle(layer, a, b, c);
      world.addArea(a, b, c, areaStiffness);
    }
    if (b !== undefined && d !== undefined && c !== undefined) {
      world.addTriangle(layer, b, d, c);
      world.addArea(b, d, c, areaStiffness);
    }
  }

  function isInsideHumanoid(nx, ny) {
    const head = ellipse(nx, ny, 0, 0.105, 0.078, 0.085);
    const neck = box(nx, ny, -0.034, 0.034, 0.17, 0.25);
    const shoulders = ellipse(nx, ny, 0, 0.275, 0.205, 0.075);
    const chest = ellipse(nx, ny, 0, 0.43, 0.155, 0.225);
    const hips = ellipse(nx, ny, 0, 0.64, 0.132, 0.11);
    const leftArm = capsule(nx, ny, -0.195, 0.285, -0.245, 0.62, 0.052);
    const rightArm = capsule(nx, ny, 0.195, 0.285, 0.245, 0.62, 0.052);
    const leftLeg = capsule(nx, ny, -0.065, 0.675, -0.082, 0.97, 0.056);
    const rightLeg = capsule(nx, ny, 0.065, 0.675, 0.082, 0.97, 0.056);
    return head || neck || shoulders || chest || hips || leftArm || rightArm || leftLeg || rightLeg;
  }

  function ellipse(x, y, cx, cy, rx, ry) {
    const dx = (x - cx) / rx;
    const dy = (y - cy) / ry;
    return dx * dx + dy * dy <= 1;
  }

  function box(x, y, minX, maxX, minY, maxY) {
    return x >= minX && x <= maxX && y >= minY && y <= maxY;
  }

  function capsule(x, y, ax, ay, bx, by, radius) {
    const abx = bx - ax;
    const aby = by - ay;
    const apx = x - ax;
    const apy = y - ay;
    const abLenSq = abx * abx + aby * aby;
    const t = Math.max(0, Math.min(1, (apx * abx + apy * aby) / abLenSq));
    const cx = ax + abx * t;
    const cy = ay + aby * t;
    const dx = x - cx;
    const dy = y - cy;
    return dx * dx + dy * dy <= radius * radius;
  }

  RP.BodyFactory = {
    createLayeredBody
  };
})(window.RP);
