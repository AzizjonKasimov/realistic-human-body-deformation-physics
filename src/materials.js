"use strict";

window.RP = window.RP || {};

window.RP.Materials = {
  config: {
    fixedDt: 1 / 60,
    solverIterations: 9,
    gravity: 920,
    damping: 0.992,
    pointSpacing: 18,
    floorFriction: 0.78,
    shapeStiffness: {
      skin: 0.006,
      muscle: 0.018
    },
    strikerRadius: 34,
    strikerMass: 2.9,
    directMuscleContact: 0.18,
    maxBoneContactsPerFrame: 120,
    maxDebugContacts: 80
  },
  skin: {
    structuralStiffness: 0.82,
    shearStiffness: 0.48,
    areaStiffness: 0.03,
    tearStretch: 1.68,
    tearImpulse: 820
  },
  muscle: {
    fiberStiffness: 0.74,
    crossStiffness: 0.34,
    shearStiffness: 0.28,
    areaStiffness: 0.24,
    tearStretch: 1.92,
    tearImpulse: 1180,
    exposedTearImpulse: 620
  },
  attachment: {
    stiffness: 0.19,
    breakStretch: 2.25,
    breakImpulse: 760
  },
  bone: {
    pointCollisionStiffness: 0.68,
    impactTransfer: 1.15,
    fractureDecay: 0.985,
    fragmentGravity: 920,
    fragmentDamping: 0.985,
    maxFragmentSpeed: 520,
    maxFragmentAngularSpeed: 5.2,
    jointStiffness: 0.64,
    jointAngleStiffness: 0.18,
    sleepSpeed: 7,
    anchorStiffness: 0.16,
    anchorDamping: 0.82,
    recoilScale: 0.72,
    bounce: 0.24,
    friction: 0.82
  }
};
