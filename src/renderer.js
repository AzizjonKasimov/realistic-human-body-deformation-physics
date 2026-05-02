"use strict";

(function initRenderer(RP) {
  class Renderer {
    constructor(canvas, ctx) {
      this.canvas = canvas;
      this.ctx = ctx;
      this.debug = {
        enabled: false
      };
    }

    draw(world, pointer) {
      const width = window.innerWidth;
      const height = window.innerHeight;
      this.ctx.clearRect(0, 0, width, height);
      this.drawRoom(width, height);
      if (world) {
        this.drawBones(world, false);
        this.drawMuscle(world);
        this.drawBones(world, true);
        this.drawSkin(world);
        this.drawAttachments(world);
        this.drawWoundEdges(world);
        this.drawStress(world);
        if (this.debug.enabled) {
          this.drawBoneJoints(world);
          this.drawDebug(world);
        }
      }
      this.drawStriker(pointer);
    }

    drawRoom(width, height) {
      const ctx = this.ctx;
      ctx.fillStyle = "#181818";
      ctx.fillRect(0, 0, width, height);
      ctx.fillStyle = "#24211d";
      ctx.fillRect(0, height - 38, width, 38);
      ctx.strokeStyle = "#4b453d";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, height - 38.5);
      ctx.lineTo(width, height - 38.5);
      ctx.stroke();
    }

    drawMuscle(world) {
      const ctx = this.ctx;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";

      for (const tri of world.muscleTriangles) {
        const a = world.points[tri.a];
        const b = world.points[tri.b];
        const c = world.points[tri.c];
        const alive = world.muscleTriangleAlive(tri);
        const exposure = (a.exposure + b.exposure + c.exposure) / 3;
        const load = Math.min(1, (a.load + b.load + c.load) / 1600);
        if (exposure < 0.04) continue;

        if (!alive) {
          ctx.fillStyle = "rgba(28, 10, 12, 0.78)";
        } else {
          const red = Math.round(90 + load * 34 + exposure * 28);
          const green = Math.round(28 - load * 5);
          const blue = Math.round(36 - load * 7);
          ctx.fillStyle = `rgb(${red}, ${green}, ${blue})`;
        }

        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.lineTo(c.x, c.y);
        ctx.closePath();
        ctx.fill();
      }

      this.drawMuscleFibers(world);
    }

    drawMuscleFibers(world) {
      const ctx = this.ctx;
      ctx.lineCap = "round";

      for (const spring of world.muscleSprings) {
        if (spring.broken || !spring.fiber) continue;
        const a = world.points[spring.a];
        const b = world.points[spring.b];
        const exposure = Math.max(a.exposure, b.exposure);
        const stress = Math.min(1, spring.stress * 1.6);
        if (exposure < 0.08 && stress < 0.08) continue;

        ctx.strokeStyle = `rgba(${Math.round(185 + stress * 45)}, ${Math.round(65 - stress * 20)}, ${Math.round(66 - stress * 24)}, ${0.24 + exposure * 0.48 + stress * 0.2})`;
        ctx.lineWidth = 1 + exposure * 0.8 + stress * 0.5;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
      }

      for (const tri of world.muscleTriangles) {
        if (world.muscleTriangleAlive(tri)) continue;
        const a = world.points[tri.a];
        const b = world.points[tri.b];
        const c = world.points[tri.c];
        const cx = (a.x + b.x + c.x) / 3;
        const cy = (a.y + b.y + c.y) / 3;
        const angle = muscleFiberAngle(cx, cy);
        const size = Math.min(15, Math.sqrt(Math.abs(RP.Physics.signedArea(a, b, c))) * 0.9);

        ctx.strokeStyle = "rgba(218, 74, 72, 0.58)";
        ctx.lineWidth = 1.2;
        for (let i = 0; i < 3; i++) {
          const offset = (i - 1) * 3.2;
          const jitter = RP.Physics.hash01(tri.seed * 317 + i * 79) - 0.5;
          const sideX = Math.sin(angle);
          const sideY = -Math.cos(angle);
          const sx = cx + sideX * offset + jitter * 2;
          const sy = cy + sideY * offset - jitter * 2;
          const dx = Math.cos(angle) * size * (0.28 + RP.Physics.hash01(tri.seed + i) * 0.24);
          const dy = Math.sin(angle) * size * (0.28 + RP.Physics.hash01(tri.seed + i) * 0.24);
          ctx.beginPath();
          ctx.moveTo(sx - dx, sy - dy);
          ctx.lineTo(sx - dx * 0.16, sy - dy * 0.16);
          ctx.moveTo(sx + dx * 0.16, sy + dy * 0.16);
          ctx.lineTo(sx + dx, sy + dy);
          ctx.stroke();
        }
      }
    }

    drawBones(world, exposedOnly) {
      const ctx = this.ctx;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      if (this.debug.enabled && exposedOnly) return;

      for (const bone of world.bones) {
        if (!bone.active) continue;
        const ends = bone.endpoints();
        const exposure = bone.dynamic ? 1 : world.boneExposureNear(bone.x, bone.y, bone.radius + 48);
        if (this.debug.enabled) {
          this.drawBoneShape(bone, ends, 0.9, true);
          continue;
        }
        if (exposedOnly && exposure < 0.18) continue;
        if (!exposedOnly && exposure >= 0.18) continue;

        const alpha = bone.dynamic ? 0.95 : 0.08 + exposure * 0.62;
        const flash = bone.flash || 0;
        ctx.strokeStyle = `rgba(${Math.round(214 + flash * 36)}, ${Math.round(205 - flash * 34)}, ${Math.round(178 - flash * 70)}, ${alpha})`;
        ctx.lineWidth = bone.radius * 2;
        ctx.beginPath();
        ctx.moveTo(ends.x1, ends.y1);
        ctx.lineTo(ends.x2, ends.y2);
        ctx.stroke();

        ctx.strokeStyle = `rgba(91, 70, 52, ${Math.min(0.75, alpha + 0.08)})`;
        ctx.lineWidth = Math.max(1.2, bone.radius * 0.32);
        ctx.beginPath();
        ctx.moveTo(ends.x1, ends.y1);
        ctx.lineTo(ends.x2, ends.y2);
        ctx.stroke();

        if (bone.dynamic) {
          this.drawFractureCaps(bone);
        }
      }
    }

    drawBoneShape(bone, ends, alpha, withHealth) {
      const ctx = this.ctx;
      const heat = Math.min(1, bone.health / Math.max(1, bone.strength));
      const flash = bone.flash || 0;
      ctx.strokeStyle = `rgba(${Math.round(214 + heat * 36 + flash * 20)}, ${Math.round(205 - heat * 95)}, ${Math.round(178 - heat * 120)}, ${alpha})`;
      ctx.lineWidth = bone.radius * 2;
      ctx.beginPath();
      ctx.moveTo(ends.x1, ends.y1);
      ctx.lineTo(ends.x2, ends.y2);
      ctx.stroke();

      ctx.strokeStyle = bone.dynamic ? "rgba(43, 29, 20, 0.95)" : "rgba(80, 62, 44, 0.9)";
      ctx.lineWidth = Math.max(1.2, bone.radius * 0.28);
      ctx.beginPath();
      ctx.moveTo(ends.x1, ends.y1);
      ctx.lineTo(ends.x2, ends.y2);
      ctx.stroke();

      if (!withHealth || bone.dynamic) return;
      const barWidth = Math.max(30, bone.halfLength * 1.2);
      const x = bone.x - barWidth * 0.5;
      const y = bone.y - bone.radius - 12;
      ctx.fillStyle = "rgba(20, 20, 20, 0.7)";
      ctx.fillRect(x, y, barWidth, 4);
      ctx.fillStyle = heat > 0.8 ? "#ff4d3d" : heat > 0.45 ? "#f0bb45" : "#79d27a";
      ctx.fillRect(x, y, barWidth * heat, 4);
    }

    drawFractureCaps(bone) {
      const ctx = this.ctx;
      const ends = bone.endpoints();
      const capRadius = Math.max(2.2, bone.radius * 0.48);
      ctx.fillStyle = "rgba(74, 46, 39, 0.95)";
      ctx.beginPath();
      ctx.arc(ends.x1, ends.y1, capRadius, 0, Math.PI * 2);
      ctx.arc(ends.x2, ends.y2, capRadius, 0, Math.PI * 2);
      ctx.fill();
    }

    drawBoneJoints(world) {
      const ctx = this.ctx;
      ctx.save();
      ctx.lineCap = "round";
      for (const joint of world.boneJoints) {
        const a = world.bones[joint.a];
        const b = world.bones[joint.b];
        if (!a || !b || !a.active || !b.active) continue;
        const aEnd = a.endpoints();
        const bEnd = b.endpoints();
        const x = (aEnd.x2 + bEnd.x1) * 0.5;
        const y = (aEnd.y2 + bEnd.y1) * 0.5;
        const heat = Math.min(1, Math.max(joint.damage, joint.stress) / Math.max(1, joint.strength));
        ctx.strokeStyle = joint.broken
          ? "rgba(255, 55, 45, 0.95)"
          : `rgba(${Math.round(110 + heat * 145)}, ${Math.round(220 - heat * 170)}, ${Math.round(130 - heat * 90)}, 0.88)`;
        ctx.lineWidth = joint.broken ? 3 : 1.5 + heat * 3;
        ctx.beginPath();
        ctx.arc(x, y, 4 + heat * 6, 0, Math.PI * 2);
        ctx.stroke();
      }
      ctx.restore();
    }

    drawSkin(world) {
      const ctx = this.ctx;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";

      for (const tri of world.skinTriangles) {
        if (!world.skinTriangleAlive(tri)) continue;
        const a = world.points[tri.a];
        const b = world.points[tri.b];
        const c = world.points[tri.c];
        const heat = Math.min(1, (a.load + b.load + c.load) / 1700);
        ctx.fillStyle = `rgb(${Math.round(174 + heat * 42)}, ${Math.round(125 - heat * 28)}, ${Math.round(96 - heat * 36)})`;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.lineTo(c.x, c.y);
        ctx.closePath();
        ctx.fill();
      }

      ctx.strokeStyle = "rgba(255, 221, 192, 0.18)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (const spring of world.skinSprings) {
        if (spring.broken) continue;
        const a = world.points[spring.a];
        const b = world.points[spring.b];
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
      }
      ctx.stroke();
    }

    drawAttachments(world) {
      const ctx = this.ctx;
      ctx.lineCap = "round";
      ctx.strokeStyle = "rgba(255, 206, 167, 0.16)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (const attachment of world.attachments) {
        if (attachment.broken || attachment.stress < 0.18) continue;
        const skin = world.points[attachment.skinPoint];
        const muscle = world.points[attachment.musclePoint];
        ctx.moveTo(skin.x, skin.y);
        ctx.lineTo(muscle.x, muscle.y);
      }
      ctx.stroke();
    }

    drawWoundEdges(world) {
      const ctx = this.ctx;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";

      for (let index = 0; index < world.skinTriangles.length; index++) {
        const tri = world.skinTriangles[index];
        if (!world.skinTriangleAlive(tri)) continue;

        for (const edge of tri.edges) {
          if (!world.isSkinWoundBoundary(index, edge)) continue;
          const [aIndex, bIndex] = edge.split(":").map(Number);
          const a = world.points[aIndex];
          const b = world.points[bIndex];
          const pulse = Math.min(1, (a.load + b.load) / 900);

          ctx.strokeStyle = `rgba(${Math.round(142 + pulse * 70)}, ${Math.round(62 - pulse * 20)}, ${Math.round(50 - pulse * 16)}, 0.92)`;
          ctx.lineWidth = 4.6;
          ctx.beginPath();
          ctx.moveTo(a.x, a.y);
          ctx.lineTo(b.x, b.y);
          ctx.stroke();

          ctx.strokeStyle = "rgba(255, 198, 165, 0.62)";
          ctx.lineWidth = 1.2;
          ctx.beginPath();
          ctx.moveTo(a.x, a.y);
          ctx.lineTo(b.x, b.y);
          ctx.stroke();
        }
      }
    }

    drawStress(world) {
      const ctx = this.ctx;
      ctx.lineCap = "round";
      for (const spring of world.skinSprings) {
        if (spring.broken || spring.stress < 0.1) continue;
        const a = world.points[spring.a];
        const b = world.points[spring.b];
        const heat = Math.min(1, spring.stress / (RP.Materials.skin.tearStretch - 1));
        ctx.strokeStyle = `rgba(255, ${Math.round(214 - heat * 120)}, ${Math.round(128 - heat * 90)}, ${0.15 + heat * 0.38})`;
        ctx.lineWidth = 1 + heat * 2;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
      }

      for (const bone of world.bones) {
        if (!bone.active || bone.dynamic || bone.health < bone.strength * 0.18) continue;
        const ends = bone.endpoints();
        const heat = Math.min(1, bone.health / bone.strength);
        ctx.strokeStyle = `rgba(255, ${Math.round(225 - heat * 160)}, ${Math.round(96 - heat * 56)}, ${0.16 + heat * 0.55})`;
        ctx.lineWidth = 2 + heat * 3;
        ctx.beginPath();
        ctx.moveTo(ends.x1, ends.y1);
        ctx.lineTo(ends.x2, ends.y2);
        ctx.stroke();
      }
    }

    drawDebug(world) {
      const ctx = this.ctx;
      ctx.save();
      ctx.font = "11px Consolas, Liberation Mono, monospace";
      ctx.textBaseline = "top";

      for (const bone of world.bones) {
        if (!bone.active) continue;
        const label = bone.dynamic ? bone.name : `${bone.name} ${Math.round(bone.health)}/${Math.round(bone.strength)}`;
        ctx.fillStyle = bone.dynamic ? "rgba(255, 240, 190, 0.9)" : "rgba(220, 235, 255, 0.82)";
        ctx.fillText(label, bone.x + 8, bone.y - 8);
      }

      for (const contact of world.debug.contacts) {
        const strength = Math.min(1, contact.impulse / 900);
        ctx.strokeStyle = `rgba(90, 210, 255, ${0.35 + strength * 0.55})`;
        ctx.fillStyle = `rgba(90, 210, 255, ${0.5 + strength * 0.45})`;
        ctx.lineWidth = 1 + strength * 3;
        ctx.beginPath();
        ctx.arc(contact.x, contact.y, 3 + strength * 7, 0, Math.PI * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(contact.x, contact.y);
        ctx.lineTo(contact.x + contact.vx * 0.35, contact.y + contact.vy * 0.35);
        ctx.stroke();
        ctx.fillText(Math.round(contact.impulse).toString(), contact.x + 8, contact.y + 4);
      }

      for (const fracture of world.debug.fractures.slice(-8)) {
        ctx.strokeStyle = "rgba(255, 64, 48, 0.85)";
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(fracture.x - 8, fracture.y - 8);
        ctx.lineTo(fracture.x + 8, fracture.y + 8);
        ctx.moveTo(fracture.x + 8, fracture.y - 8);
        ctx.lineTo(fracture.x - 8, fracture.y + 8);
        ctx.stroke();
      }

      ctx.restore();
    }

    drawStriker(pointer) {
      if (!pointer.active) return;
      const ctx = this.ctx;
      const radius = RP.Materials.config.strikerRadius;
      ctx.fillStyle = pointer.down ? "rgba(226, 226, 218, 0.34)" : "rgba(226, 226, 218, 0.2)";
      ctx.strokeStyle = "#eee8da";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(pointer.x, pointer.y, radius, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
  }

  function muscleFiberAngle(x, y) {
    const width = window.innerWidth;
    const height = window.innerHeight;
    const centerX = width * 0.52;
    const t = y / height;
    if (t < 0.25) return Math.PI / 2;
    if (t > 0.68) return Math.PI / 2 + (x < centerX ? -0.06 : 0.06);
    if (Math.abs(x - centerX) > 100 && t < 0.68) return Math.PI / 2 + (x < centerX ? -0.14 : 0.14);
    return Math.PI / 2 + (x < centerX ? 0.08 : -0.08);
  }

  RP.Renderer = {
    Renderer
  };
})(window.RP);
