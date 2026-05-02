"use strict";

(function initPhysics(RP) {
  class Point {
    constructor(x, y, layer, pinned = false) {
      this.x = x;
      this.y = y;
      this.oldX = x;
      this.oldY = y;
      this.homeX = x;
      this.homeY = y;
      this.layer = layer;
      this.pinned = pinned;
      this.load = 0;
      this.exposure = 0;
      this.mass = layer === "muscle" ? 1.25 : 1;
    }
  }

  class DistanceConstraint {
    constructor(world, a, b, options) {
      this.a = a;
      this.b = b;
      this.rest = distance(world.points[a], world.points[b]);
      this.stiffness = options.stiffness;
      this.tearStretch = options.tearStretch;
      this.tearImpulse = options.tearImpulse;
      this.kind = options.kind;
      this.breakable = options.breakable !== false;
      this.broken = false;
      this.stress = 0;
      this.fiber = Boolean(options.fiber);
    }
  }

  class AreaConstraint {
    constructor(world, a, b, c, stiffness) {
      this.a = a;
      this.b = b;
      this.c = c;
      this.restArea = signedArea(world.points[a], world.points[b], world.points[c]);
      this.stiffness = stiffness;
    }
  }

  class AttachmentConstraint {
    constructor(world, skinPoint, musclePoint, options) {
      this.skinPoint = skinPoint;
      this.musclePoint = musclePoint;
      this.rest = distance(world.points[skinPoint], world.points[musclePoint]);
      this.stiffness = options.stiffness;
      this.breakStretch = options.breakStretch;
      this.breakImpulse = options.breakImpulse;
      this.broken = false;
      this.stress = 0;
    }
  }

  class BonePiece {
    constructor(name, x1, y1, x2, y2, radius, strength, dynamic = false, parentName = name, segmentIndex = 0, segmentCount = 1) {
      const dx = x2 - x1;
      const dy = y2 - y1;
      this.name = name;
      this.parentName = parentName;
      this.segmentIndex = segmentIndex;
      this.segmentCount = segmentCount;
      this.x = (x1 + x2) * 0.5;
      this.y = (y1 + y2) * 0.5;
      this.angle = Math.atan2(dy, dx);
      this.baseX = this.x;
      this.baseY = this.y;
      this.baseAngle = this.angle;
      this.halfLength = Math.max(6, Math.hypot(dx, dy) * 0.5);
      this.radius = radius;
      this.strength = strength;
      this.health = 0;
      this.dynamic = dynamic;
      this.active = true;
      this.vx = 0;
      this.vy = 0;
      this.angularVelocity = 0;
      this.flash = 0;
      this.seed = hash01(x1 * 17 + y1 * 29 + x2 * 37 + y2 * 43);
    }

    endpoints() {
      const dx = Math.cos(this.angle) * this.halfLength;
      const dy = Math.sin(this.angle) * this.halfLength;
      return {
        x1: this.x - dx,
        y1: this.y - dy,
        x2: this.x + dx,
        y2: this.y + dy
      };
    }
  }

  class BoneJoint {
    constructor(parentName, a, b, strength) {
      this.parentName = parentName;
      this.a = a;
      this.b = b;
      this.strength = strength;
      this.damage = 0;
      this.stress = 0;
      this.broken = false;
      this.flash = 0;
    }
  }

  class World {
    constructor(materials) {
      this.materials = materials;
      this.config = materials.config;
      this.points = [];
      this.skinSprings = [];
      this.muscleSprings = [];
      this.skinSpringByEdge = new Map();
      this.muscleSpringByEdge = new Map();
      this.areaConstraints = [];
      this.attachments = [];
      this.skinTriangles = [];
      this.muscleTriangles = [];
      this.bones = [];
      this.boneJoints = [];
      this.boneContactCandidates = [];
      this.skinEdgeToTriangles = new Map();
      this.muscleEdgeToTriangles = new Map();
      this.skinToMuscle = new Map();
      this.debug = {
        contacts: [],
        fractures: []
      };
      this.stats = {
        brokenSkin: 0,
        brokenMuscle: 0,
        brokenAttachments: 0,
        boneFractures: 0
      };
    }

    addPoint(x, y, layer, pinned = false) {
      const index = this.points.length;
      this.points.push(new Point(x, y, layer, pinned));
      return index;
    }

    addSkinSpring(a, b, options) {
      const edge = edgeKey(a, b);
      if (this.skinSpringByEdge.has(edge)) return this.skinSpringByEdge.get(edge);
      const spring = new DistanceConstraint(this, a, b, {
        kind: "skin",
        breakable: true,
        ...options
      });
      this.skinSprings.push(spring);
      this.skinSpringByEdge.set(edge, spring);
      return spring;
    }

    addMuscleSpring(a, b, options) {
      const edge = edgeKey(a, b);
      if (this.muscleSpringByEdge.has(edge)) return this.muscleSpringByEdge.get(edge);
      const spring = new DistanceConstraint(this, a, b, {
        kind: "muscle",
        breakable: true,
        ...options
      });
      this.muscleSprings.push(spring);
      this.muscleSpringByEdge.set(edge, spring);
      return spring;
    }

    addArea(a, b, c, stiffness) {
      this.areaConstraints.push(new AreaConstraint(this, a, b, c, stiffness));
    }

    addAttachment(skinPoint, musclePoint) {
      this.skinToMuscle.set(skinPoint, musclePoint);
      this.attachments.push(new AttachmentConstraint(this, skinPoint, musclePoint, this.materials.attachment));
    }

    addBone(name, x1, y1, x2, y2, radius, strength) {
      const length = Math.hypot(x2 - x1, y2 - y1);
      const segmentCount = Math.max(2, Math.min(8, Math.ceil(length / 26)));
      const startIndex = this.bones.length;
      for (let i = 0; i < segmentCount; i++) {
        const t0 = i / segmentCount;
        const t1 = (i + 1) / segmentCount;
        const ax = x1 + (x2 - x1) * t0;
        const ay = y1 + (y2 - y1) * t0;
        const bx = x1 + (x2 - x1) * t1;
        const by = y1 + (y2 - y1) * t1;
        this.bones.push(new BonePiece(`${name} ${i + 1}`, ax, ay, bx, by, radius, strength, false, name, i, segmentCount));
      }
      for (let i = 0; i < segmentCount - 1; i++) {
        this.boneJoints.push(new BoneJoint(name, startIndex + i, startIndex + i + 1, strength));
      }
      return this.bones[startIndex];
    }

    addTriangle(layer, a, b, c) {
      const tri = {
        a,
        b,
        c,
        edges: [edgeKey(a, b), edgeKey(b, c), edgeKey(c, a)],
        damage: 0,
        failed: false,
        seed: hash01(a * 928371 + b * 689287 + c * 283923)
      };
      const list = layer === "skin" ? this.skinTriangles : this.muscleTriangles;
      const edgeMap = layer === "skin" ? this.skinEdgeToTriangles : this.muscleEdgeToTriangles;
      const index = list.length;
      list.push(tri);
      for (const edge of tri.edges) {
        if (!edgeMap.has(edge)) edgeMap.set(edge, []);
        edgeMap.get(edge).push(index);
      }
      return tri;
    }

    step(dt, input) {
      const floorY = window.innerHeight - 38;
      this.debug.contacts.length = 0;
      this.markMuscleExposure();
      this.integrateBones(dt, floorY);
      this.integrate(dt, floorY);
      this.collideStriker(dt, input);
      this.collectBoneContactCandidates();

      for (let iteration = 0; iteration < this.config.solverIterations; iteration++) {
        this.solveSprings(this.skinSprings);
        this.solveSprings(this.muscleSprings);
        this.solveAttachments();
        this.solveBoneContacts();
        this.solveBoneJoints();
        this.solveAreas();
        this.constrainToWorld(floorY);
      }

      this.updateTriangleDamage();
      this.updateBoneJointDamage();
    }

    integrateBones(dt, floorY) {
      const boneConfig = this.materials.bone;
      for (const bone of this.bones) {
        if (!bone.active) continue;
        bone.health *= boneConfig.fractureDecay;
        bone.flash *= 0.88;
        if (bone.dynamic) {
          if (bone.sleeping) continue;
          bone.vx *= boneConfig.fragmentDamping;
          bone.vy = bone.vy * boneConfig.fragmentDamping + boneConfig.fragmentGravity * dt;
          bone.angularVelocity *= boneConfig.fragmentDamping;
          clampBoneVelocity(bone, boneConfig.maxFragmentSpeed, boneConfig.maxFragmentAngularSpeed);
        } else {
          bone.vx = bone.vx * boneConfig.anchorDamping + (bone.baseX - bone.x) * boneConfig.anchorStiffness;
          bone.vy = bone.vy * boneConfig.anchorDamping + (bone.baseY - bone.y) * boneConfig.anchorStiffness;
          bone.angularVelocity = bone.angularVelocity * boneConfig.anchorDamping + angleDifference(bone.baseAngle, bone.angle) * boneConfig.anchorStiffness;
        }
        bone.x += bone.vx * dt;
        bone.y += bone.vy * dt;
        bone.angle += bone.angularVelocity * dt;

        const bottom = boneCapsuleBottom(bone);
        if (bone.dynamic && bottom > floorY) {
          bone.y -= bottom - floorY;
          bone.vy *= -boneConfig.bounce;
          bone.vx *= boneConfig.friction;
          bone.angularVelocity *= boneConfig.friction;
          if (Math.hypot(bone.vx, bone.vy) < boneConfig.sleepSpeed && Math.abs(bone.angularVelocity) < 0.08) {
            bone.sleeping = true;
            bone.vx = 0;
            bone.vy = 0;
            bone.angularVelocity = 0;
          }
        }
      }

      for (const joint of this.boneJoints) {
        joint.damage *= boneConfig.fractureDecay;
        joint.flash *= 0.86;
      }
    }

    integrate(dt, floorY) {
      for (const point of this.points) {
        point.load *= 0.84;
        point.exposure *= 0.92;
        if (point.pinned) {
          point.x = point.homeX;
          point.y = point.homeY;
          point.oldX = point.x;
          point.oldY = point.y;
          continue;
        }

        const vx = (point.x - point.oldX) * this.config.damping;
        const vy = (point.y - point.oldY) * this.config.damping;
        point.oldX = point.x;
        point.oldY = point.y;
        point.x += vx;
        point.y += vy + this.config.gravity * dt * dt;

        const stiffness = this.config.shapeStiffness[point.layer];
        point.x += (point.homeX - point.x) * stiffness;
        point.y += (point.homeY - point.y) * stiffness;

        if (point.y > floorY) {
          point.y = floorY;
          point.oldX = point.x + (point.oldX - point.x) * this.config.floorFriction;
        }
      }

    }

    collideStrikerWithBones(input, impact, influence) {
      if (!input.down) return;
      for (const bone of this.bones) {
        if (!bone.active) continue;
        const closest = closestPointOnBone(bone, input.x, input.y);
        const dx = input.x - closest.x;
        const dy = input.y - closest.y;
        const dist = Math.hypot(dx, dy);
        const reach = influence + bone.radius;
        if (dist > reach) continue;

        const exposure = this.boneExposureNear(closest.x, closest.y, bone.radius + 48);
        const transfer = this.materials.bone.impactTransfer * (0.28 + exposure * 0.95);
        const delivered = impact * (1 - dist / reach) * transfer;
        bone.health += delivered;
        bone.sleeping = false;
        bone.flash = Math.min(1, bone.flash + delivered / Math.max(1, bone.strength));
        bone.vx += input.vx * transfer * this.materials.bone.recoilScale;
        bone.vy += input.vy * transfer * this.materials.bone.recoilScale;
        bone.angularVelocity += ((closest.x - bone.x) * input.vy - (closest.y - bone.y) * input.vx) * 0.0012 * transfer;
        this.pushDebugContact({
          type: "striker",
          bone: bone.parentName,
          segment: bone.name,
          x: closest.x,
          y: closest.y,
          impulse: delivered,
          vx: input.vx,
          vy: input.vy
        });
        this.addJointImpact(bone, delivered, closest.t);

        if (bone.dynamic) bone.angularVelocity += ((closest.x - bone.x) * input.vy - (closest.y - bone.y) * input.vx) * 0.0009;
      }
    }

    collideStriker(dt, input) {
      if (!input.active) return;

      const speed = Math.hypot(input.vx, input.vy);
      const power = input.power || 1;
      const impact = speed * this.config.strikerMass * power;
      const influence = this.config.strikerRadius + 12;

      for (const point of this.points) {
        if (point.pinned) continue;
        const dx = point.x - input.x;
        const dy = point.y - input.y;
        const dist = Math.hypot(dx, dy);
        if (dist > influence || dist < 0.0001) continue;

        let contactStrength = (input.down ? 0.74 : 0.2) * (0.85 + power * 0.15);
        if (point.layer === "muscle") {
          contactStrength *= this.config.directMuscleContact + point.exposure * 0.82;
        }

        const nx = dx / dist;
        const ny = dy / dist;
        const depth = influence - dist;
        point.x += nx * depth * contactStrength + input.vx * dt * 0.45 * contactStrength;
        point.y += ny * depth * contactStrength + input.vy * dt * 0.45 * contactStrength;
        point.load = Math.max(point.load, impact * (depth / influence) * contactStrength);
      }

      this.collideStrikerWithBones(input, impact, influence);
    }

    collectBoneContactCandidates() {
      this.boneContactCandidates.length = 0;
      const limit = this.config.maxBoneContactsPerFrame;
      for (const point of this.points) {
        if (point.pinned) continue;
        if (point.load < 150 && point.exposure < 0.16) continue;
        this.boneContactCandidates.push(point);
        if (this.boneContactCandidates.length >= limit) break;
      }
    }

    solveSprings(springs) {
      for (const spring of springs) {
        if (spring.broken) continue;
        const a = this.points[spring.a];
        const b = this.points[spring.b];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const len = Math.hypot(dx, dy);
        if (len < 0.0001) continue;

        const stretchRatio = len / spring.rest;
        const endpointLoad = Math.max(a.load, b.load);
        const tearImpulse = spring.kind === "muscle"
          ? spring.tearImpulse * (1 - Math.max(a.exposure, b.exposure) * 0.48)
          : spring.tearImpulse;
        spring.stress = Math.max(spring.stress * 0.9, Math.max(0, stretchRatio - 1));

        if (spring.breakable && (stretchRatio > spring.tearStretch || endpointLoad > tearImpulse && stretchRatio > 1.12)) {
          spring.broken = true;
          if (spring.kind === "skin") this.stats.brokenSkin++;
          if (spring.kind === "muscle") this.stats.brokenMuscle++;
          a.load = Math.max(a.load, endpointLoad * 0.35);
          b.load = Math.max(b.load, endpointLoad * 0.35);
          continue;
        }

        const diff = (len - spring.rest) / len;
        const correctionX = dx * diff * spring.stiffness;
        const correctionY = dy * diff * spring.stiffness;
        applyPairCorrection(a, b, correctionX, correctionY);
      }
    }

    solveAttachments() {
      for (const attachment of this.attachments) {
        if (attachment.broken) continue;
        const skin = this.points[attachment.skinPoint];
        const muscle = this.points[attachment.musclePoint];
        const dx = muscle.x - skin.x;
        const dy = muscle.y - skin.y;
        const len = Math.hypot(dx, dy);
        if (len < 0.0001) continue;

        const stretchRatio = len / Math.max(1, attachment.rest);
        const impulse = Math.max(skin.load, muscle.load);
        attachment.stress = Math.max(attachment.stress * 0.88, Math.max(0, stretchRatio - 1));
        if (stretchRatio > attachment.breakStretch || impulse > attachment.breakImpulse && stretchRatio > 1.25) {
          attachment.broken = true;
          this.stats.brokenAttachments++;
          skin.exposure = Math.max(skin.exposure, 1);
          muscle.exposure = Math.max(muscle.exposure, 1);
          continue;
        }

        const diff = (len - attachment.rest) / len;
        const correctionX = dx * diff * attachment.stiffness;
        const correctionY = dy * diff * attachment.stiffness;
        applyPairCorrection(skin, muscle, correctionX, correctionY);
      }
    }

    solveBoneContacts() {
      const boneConfig = this.materials.bone;
      for (const bone of this.bones) {
        if (!bone.active) continue;
        if (bone.dynamic && bone.sleeping) continue;
        const broadRadius = bone.halfLength + bone.radius + 12;
        for (const point of this.boneContactCandidates) {
          if (point.pinned) continue;
          if (!bone.dynamic && point.load < 180 && point.exposure < 0.12) continue;
          if (Math.abs(point.x - bone.x) > broadRadius || Math.abs(point.y - bone.y) > broadRadius) continue;
          const contact = closestPointOnBone(bone, point.x, point.y);
          const dx = point.x - contact.x;
          const dy = point.y - contact.y;
          const dist = Math.hypot(dx, dy);
          const pointRadius = point.layer === "muscle" ? 4.5 : 3.2;
          const allowed = bone.radius + pointRadius;
          if (dist >= allowed || dist < 0.0001) continue;

          const nx = dx / dist;
          const ny = dy / dist;
          const depth = allowed - dist;
          const pointShare = bone.dynamic ? 0.62 : Math.min(1, point.load / 520 + point.exposure);
          point.x += nx * depth * boneConfig.pointCollisionStiffness * pointShare;
          point.y += ny * depth * boneConfig.pointCollisionStiffness * pointShare;

          const impulse = Math.max(0, Math.hypot(point.x - point.oldX, point.y - point.oldY) * 34 + point.load * 0.08 - 28);
          if (point.load > 260 || bone.dynamic) {
            bone.health += impulse * (point.layer === "muscle" ? 0.14 : 0.05);
            bone.flash = Math.max(bone.flash, Math.min(1, impulse / Math.max(1, bone.strength * 0.45)));
            if (impulse > 8) {
              this.pushDebugContact({
                type: "tissue",
                bone: bone.parentName,
                segment: bone.name,
                x: contact.x,
                y: contact.y,
                impulse,
                vx: point.x - point.oldX,
                vy: point.y - point.oldY
              });
            }
            this.addJointImpact(bone, impulse * 0.4, contact.t);
          }

          if (bone.dynamic) {
            bone.vx -= nx * depth * 16;
            bone.vy -= ny * depth * 16;
            bone.angularVelocity -= ((contact.x - bone.x) * ny - (contact.y - bone.y) * nx) * depth * 0.035;
          }
        }
      }
    }

    pushDebugContact(contact) {
      if (this.debug.contacts.length >= this.config.maxDebugContacts) return;
      this.debug.contacts.push(contact);
    }

    solveBoneJoints() {
      const boneConfig = this.materials.bone;
      for (const joint of this.boneJoints) {
        if (joint.broken) continue;
        const a = this.bones[joint.a];
        const b = this.bones[joint.b];
        if (!a.active || !b.active) continue;
        const aEnd = a.endpoints();
        const bEnd = b.endpoints();
        const dx = bEnd.x1 - aEnd.x2;
        const dy = bEnd.y1 - aEnd.y2;
        const correctionX = dx * boneConfig.jointStiffness * 0.5;
        const correctionY = dy * boneConfig.jointStiffness * 0.5;
        a.x += correctionX;
        a.y += correctionY;
        b.x -= correctionX;
        b.y -= correctionY;

        const angleDelta = angleDifference(a.angle, b.angle);
        a.angle -= angleDelta * boneConfig.jointAngleStiffness * 0.5;
        b.angle += angleDelta * boneConfig.jointAngleStiffness * 0.5;
      }
    }


    addJointImpact(bone, impulse, tOnSegment) {
      if (bone.dynamic) return;
      for (const joint of this.boneJoints) {
        if (joint.broken) continue;
        const a = this.bones[joint.a];
        const b = this.bones[joint.b];
        if (a !== bone && b !== bone) continue;
        const nearJoint = a === bone ? tOnSegment : 1 - tOnSegment;
        const locality = 0.35 + nearJoint * 0.65;
        joint.damage += impulse * locality;
        joint.flash = Math.min(1, joint.flash + impulse / Math.max(1, joint.strength));
      }
    }

    solveAreas() {
      for (const area of this.areaConstraints) {
        const a = this.points[area.a];
        const b = this.points[area.b];
        const c = this.points[area.c];
        if (a.layer === "skin" && !this.skinTriangleAliveByPoints(area.a, area.b, area.c)) continue;
        if (a.layer === "muscle" && !this.muscleTriangleUsableByPoints(area.a, area.b, area.c)) continue;

        const current = signedArea(a, b, c);
        const constraint = current - area.restArea;
        const invMassA = a.pinned ? 0 : 1 / a.mass;
        const invMassB = b.pinned ? 0 : 1 / b.mass;
        const invMassC = c.pinned ? 0 : 1 / c.mass;

        const ax = b.y - c.y;
        const ay = c.x - b.x;
        const bx = c.y - a.y;
        const by = a.x - c.x;
        const cx = a.y - b.y;
        const cy = b.x - a.x;
        const weightedGradient =
          invMassA * (ax * ax + ay * ay) +
          invMassB * (bx * bx + by * by) +
          invMassC * (cx * cx + cy * cy);
        if (weightedGradient <= 0.0001) continue;

        const lambda = -constraint * area.stiffness / weightedGradient;

        if (!a.pinned) {
          a.x += ax * lambda * invMassA;
          a.y += ay * lambda * invMassA;
        }
        if (!b.pinned) {
          b.x += bx * lambda * invMassB;
          b.y += by * lambda * invMassB;
        }
        if (!c.pinned) {
          c.x += cx * lambda * invMassC;
          c.y += cy * lambda * invMassC;
        }
      }
    }

    constrainToWorld(floorY) {
      const margin = 8;
      const width = window.innerWidth;
      for (const point of this.points) {
        if (point.pinned) continue;
        if (point.x < margin) point.x = margin;
        if (point.x > width - margin) point.x = width - margin;
        if (point.y > floorY) point.y = floorY;
      }
    }

    updateTriangleDamage() {
      for (const tri of this.muscleTriangles) {
        if (tri.failed) continue;
        const a = this.points[tri.a];
        const b = this.points[tri.b];
        const c = this.points[tri.c];
        const load = (a.load + b.load + c.load) / 3;
        const exposed = (a.exposure + b.exposure + c.exposure) / 3;
        const strain = this.averageMuscleEdgeStrain(tri);
        const impulseThreshold = this.materials.muscle.exposedTearImpulse + (1 - exposed) * 560;
        tri.damage = Math.min(1.35, tri.damage * 0.996 + Math.max(0, load - impulseThreshold) / 1500 + Math.max(0, strain - 1.08) * 0.035);
        if (tri.damage > 1) tri.failed = true;
      }
    }

    updateBoneJointDamage() {
      for (const joint of this.boneJoints) {
        if (joint.broken) continue;
        const a = this.bones[joint.a];
        const b = this.bones[joint.b];
        if (!a.active || !b.active) continue;
        const aEnd = a.endpoints();
        const bEnd = b.endpoints();
        const gap = Math.hypot(bEnd.x1 - aEnd.x2, bEnd.y1 - aEnd.y2);
        const bend = Math.abs(angleDifference(a.angle, b.angle));
        const shear = Math.abs((b.x - a.x) * Math.sin(a.angle) - (b.y - a.y) * Math.cos(a.angle));
        const stress = gap * 5 + bend * 220 + shear * 1.3 + Math.max(a.health, b.health) * 0.08;
        joint.stress = Math.max(joint.stress * 0.92, stress);
        joint.damage += Math.max(0, stress - joint.strength * 0.28) * 0.04;
        if (joint.damage >= joint.strength) this.fractureJoint(joint);
      }
    }

    fractureJoint(joint) {
      joint.broken = true;
      const a = this.bones[joint.a];
      const b = this.bones[joint.b];
      const aEnd = a.endpoints();
      const bEnd = b.endpoints();
      const breakX = (aEnd.x2 + bEnd.x1) * 0.5;
      const breakY = (aEnd.y2 + bEnd.y1) * 0.5;
      const kick = Math.min(260, joint.damage / Math.max(1, joint.strength) * 120);
      for (const segment of this.bones) {
        if (segment.parentName === joint.parentName) segment.dynamic = true;
      }
      a.vx -= Math.sin(a.angle) * kick * 0.24;
      a.vy += Math.cos(a.angle) * kick * 0.16;
      b.vx += Math.sin(b.angle) * kick * 0.24;
      b.vy -= Math.cos(b.angle) * kick * 0.16;
      a.angularVelocity -= 1.1 + a.seed * 0.7;
      b.angularVelocity += 1.1 + b.seed * 0.7;
      a.flash = 1;
      b.flash = 1;
      this.debug.fractures.push({
        bone: joint.parentName,
        x: breakX,
        y: breakY,
        health: joint.damage,
        strength: joint.strength,
        fragmentNames: [a.name, b.name]
      });
      this.stats.boneFractures++;
    }

    markMuscleExposure() {
      for (const tri of this.skinTriangles) {
        if (this.skinTriangleAlive(tri)) continue;
        for (const skinIndex of [tri.a, tri.b, tri.c]) {
          const muscleIndex = this.skinToMuscle.get(skinIndex);
          if (muscleIndex !== undefined) {
            this.points[muscleIndex].exposure = Math.max(this.points[muscleIndex].exposure, 1);
          }
        }
      }
    }

    averageMuscleEdgeStrain(tri) {
      let total = 0;
      let count = 0;
      for (const edge of tri.edges) {
        const spring = this.muscleSpringByEdge.get(edge);
        if (!spring) continue;
        total += distance(this.points[spring.a], this.points[spring.b]) / spring.rest;
        count++;
      }
      return count === 0 ? 1 : total / count;
    }

    boneExposureNear(x, y, radius) {
      let exposed = 0;
      let count = 0;
      const radiusSq = radius * radius;
      for (const point of this.points) {
        if (point.layer !== "muscle") continue;
        const dx = point.x - x;
        const dy = point.y - y;
        const distSq = dx * dx + dy * dy;
        if (distSq > radiusSq) continue;
        const weight = 1 - distSq / radiusSq;
        exposed += point.exposure * weight;
        count += weight;
      }
      return count <= 0 ? 0 : Math.min(1, exposed / count);
    }

    skinTriangleAlive(tri) {
      return tri.edges.every((edge) => {
        const spring = this.skinSpringByEdge.get(edge);
        return spring && !spring.broken;
      });
    }

    muscleTriangleAlive(tri) {
      if (tri.failed) return false;
      return tri.edges.every((edge) => {
        const spring = this.muscleSpringByEdge.get(edge);
        return spring && !spring.broken;
      });
    }

    skinTriangleAliveByPoints(a, b, c) {
      return [edgeKey(a, b), edgeKey(b, c), edgeKey(c, a)].every((edge) => {
        const spring = this.skinSpringByEdge.get(edge);
        return spring && !spring.broken;
      });
    }

    muscleTriangleUsableByPoints(a, b, c) {
      return [edgeKey(a, b), edgeKey(b, c), edgeKey(c, a)].every((edge) => {
        const spring = this.muscleSpringByEdge.get(edge);
        return spring && !spring.broken;
      });
    }

    isSkinWoundBoundary(triangleIndex, edge) {
      const neighbors = this.skinEdgeToTriangles.get(edge);
      if (!neighbors || neighbors.length < 2) return false;
      for (const neighborIndex of neighbors) {
        if (neighborIndex === triangleIndex) continue;
        if (!this.skinTriangleAlive(this.skinTriangles[neighborIndex])) return true;
      }
      return false;
    }
  }

  function applyPairCorrection(a, b, correctionX, correctionY) {
    const invMassA = a.pinned ? 0 : 1 / a.mass;
    const invMassB = b.pinned ? 0 : 1 / b.mass;
    const total = invMassA + invMassB;
    if (total <= 0) return;
    if (!a.pinned) {
      a.x += correctionX * invMassA / total;
      a.y += correctionY * invMassA / total;
    }
    if (!b.pinned) {
      b.x -= correctionX * invMassB / total;
      b.y -= correctionY * invMassB / total;
    }
  }

  function closestPointOnBone(bone, x, y) {
    const ends = bone.endpoints();
    const abx = ends.x2 - ends.x1;
    const aby = ends.y2 - ends.y1;
    const apx = x - ends.x1;
    const apy = y - ends.y1;
    const abLenSq = abx * abx + aby * aby;
    const t = abLenSq <= 0.0001 ? 0 : Math.max(0, Math.min(1, (apx * abx + apy * aby) / abLenSq));
    return {
      x: ends.x1 + abx * t,
      y: ends.y1 + aby * t,
      t
    };
  }

  function boneCapsuleBottom(bone) {
    const ends = bone.endpoints();
    return Math.max(ends.y1, ends.y2) + bone.radius;
  }

  function clampBoneVelocity(bone, maxSpeed, maxAngularSpeed) {
    const speed = Math.hypot(bone.vx, bone.vy);
    if (speed > maxSpeed) {
      const scale = maxSpeed / speed;
      bone.vx *= scale;
      bone.vy *= scale;
    }
    bone.angularVelocity = Math.max(-maxAngularSpeed, Math.min(maxAngularSpeed, bone.angularVelocity));
  }

  function angleDifference(target, value) {
    return Math.atan2(Math.sin(target - value), Math.cos(target - value));
  }

  function distance(a, b) {
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    return Math.hypot(dx, dy);
  }

  function signedArea(a, b, c) {
    return ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)) * 0.5;
  }

  function edgeKey(a, b) {
    return a < b ? `${a}:${b}` : `${b}:${a}`;
  }

  function hash01(value) {
    const x = Math.sin(value * 12.9898) * 43758.5453;
    return x - Math.floor(x);
  }

  RP.Physics = {
    World,
    BonePiece,
    distance,
    edgeKey,
    closestPointOnBone,
    hash01,
    signedArea
  };
})(window.RP);
