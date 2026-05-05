#include "simulation.hpp"

#include <algorithm>
#include <cmath>
#include <iostream>

namespace {

int fail(const char* message) {
    std::cerr << "FAIL: " << message << '\n';
    return 1;
}

double testBoneAngle(const rp::BoneSegment& bone) {
    return std::atan2(bone.b.y - bone.a.y, bone.b.x - bone.a.x);
}

rp::Vec2 testBonePoint(const rp::BoneSegment& bone, double t) {
    return {
        bone.a.x + (bone.b.x - bone.a.x) * t,
        bone.a.y + (bone.b.y - bone.a.y) * t,
    };
}

double jointAnchorDistance(const rp::World& world, const rp::BoneJoint& joint) {
    return rp::distance(testBonePoint(world.bones()[joint.a], joint.tA), testBonePoint(world.bones()[joint.b], joint.tB));
}

double angleDelta(double a, double b) {
    constexpr double pi = 3.14159265358979323846;
    double delta = std::fmod(std::abs(a - b), pi * 2.0);
    if (delta > pi) {
        delta = pi * 2.0 - delta;
    }
    return delta;
}

double jointAngleViolation(const rp::World& world, const rp::BoneJoint& joint) {
    constexpr double pi = 3.14159265358979323846;
    const double restAngle = joint.postFractureRest > 0.0 ? joint.postFractureRestAngle : joint.restAngle;
    double relativeAngle = testBoneAngle(world.bones()[joint.b]) - testBoneAngle(world.bones()[joint.a]) - restAngle;
    while (relativeAngle > pi) {
        relativeAngle -= pi * 2.0;
    }
    while (relativeAngle < -pi) {
        relativeAngle += pi * 2.0;
    }
    const double minAngle = joint.minAngle - world.materials().postFractureJointAngleSlack;
    const double maxAngle = joint.maxAngle + world.materials().postFractureJointAngleSlack;
    return std::max(0.0, std::max(minAngle - relativeAngle, relativeAngle - maxAngle));
}

rp::Vec2 testLerp(rp::Vec2 a, rp::Vec2 b, double t) {
    return {a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t};
}

double testDot(rp::Vec2 a, rp::Vec2 b) {
    return a.x * b.x + a.y * b.y;
}

rp::Vec2 testSubtract(rp::Vec2 a, rp::Vec2 b) {
    return {a.x - b.x, a.y - b.y};
}

double testSegmentDistance(rp::Vec2 a0, rp::Vec2 a1, rp::Vec2 b0, rp::Vec2 b1) {
    constexpr double epsilon = 0.0001;
    const rp::Vec2 dA = testSubtract(a1, a0);
    const rp::Vec2 dB = testSubtract(b1, b0);
    const rp::Vec2 r = testSubtract(a0, b0);
    const double lenA = testDot(dA, dA);
    const double lenB = testDot(dB, dB);
    const double dBF = testDot(dB, r);
    double tA = 0.0;
    double tB = 0.0;

    if (lenA <= epsilon && lenB <= epsilon) {
        tA = 0.0;
        tB = 0.0;
    } else if (lenA <= epsilon) {
        tA = 0.0;
        tB = std::clamp(dBF / lenB, 0.0, 1.0);
    } else {
        const double dAC = testDot(dA, r);
        if (lenB <= epsilon) {
            tB = 0.0;
            tA = std::clamp(-dAC / lenA, 0.0, 1.0);
        } else {
            const double dAB = testDot(dA, dB);
            const double denom = lenA * lenB - dAB * dAB;
            tA = denom > epsilon ? std::clamp((dAB * dBF - dAC * lenB) / denom, 0.0, 1.0) : 0.0;

            const double tBNumer = dAB * tA + dBF;
            if (tBNumer < 0.0) {
                tB = 0.0;
                tA = std::clamp(-dAC / lenA, 0.0, 1.0);
            } else if (tBNumer > lenB) {
                tB = 1.0;
                tA = std::clamp((dAB - dAC) / lenA, 0.0, 1.0);
            } else {
                tB = tBNumer / lenB;
            }
        }
    }

    return rp::distance(testLerp(a0, a1, tA), testLerp(b0, b1, tB));
}

double maxFreeFragmentOverlap(const rp::World& world) {
    double maxOverlap = 0.0;
    for (std::size_t i = 0; i < world.bones().size(); ++i) {
        const rp::BoneSegment& a = world.bones()[i];
        if (a.pinned || (!a.fractured && !a.splinter)) {
            continue;
        }
        for (std::size_t j = i + 1; j < world.bones().size(); ++j) {
            const rp::BoneSegment& b = world.bones()[j];
            if (b.pinned || (!b.fractured && !b.splinter)) {
                continue;
            }
            const double surfaceOverlap = a.radius + b.radius - testSegmentDistance(a.a, a.b, b.a, b.b);
            maxOverlap = std::max(maxOverlap, surfaceOverlap);
        }
    }
    return maxOverlap;
}

int activeWoundCount(const rp::World& world) {
    return static_cast<int>(std::count_if(world.wounds().begin(), world.wounds().end(), [](const rp::WoundSource& wound) {
        return wound.active;
    }));
}

} // namespace

int main() {
    rp::World world = rp::createLayeredBody(1280.0, 720.0);

    if (world.points().empty()) {
        return fail("body should contain points");
    }
    if (world.springs().empty()) {
        return fail("body should contain springs");
    }
    if (world.triangles().empty()) {
        return fail("body should contain triangles");
    }
    if (world.bones().empty()) {
        return fail("body should contain bones");
    }
    if (world.boneAttachments().empty()) {
        return fail("muscle should be attached to bones");
    }
    if (world.boneJoints().empty()) {
        return fail("bones should be connected by joints");
    }

    const int initialLiveSkinTriangles = static_cast<int>(std::count_if(world.triangles().begin(), world.triangles().end(), [&](const rp::Triangle& triangle) {
        return triangle.layer == rp::TissueLayer::Skin && world.triangleAlive(triangle);
    }));
    const std::size_t initialBoneCount = world.bones().size();
    const int skinPoints = static_cast<int>(std::count_if(world.points().begin(), world.points().end(), [](const rp::Point& point) {
        return point.layer == rp::TissueLayer::Skin;
    }));
    const int musclePoints = static_cast<int>(std::count_if(world.points().begin(), world.points().end(), [](const rp::Point& point) {
        return point.layer == rp::TissueLayer::Muscle;
    }));
    if (musclePoints >= skinPoints) {
        return fail("muscle layer should be an inner subset of the skin layer");
    }
    const rp::AnatomyValidation anatomy = rp::validateAnatomy(world);
    if (anatomy.boneSamplesOutsideSkin != 0) {
        return fail("bone centerlines should stay inside the skin layer");
    }

    for (int i = 0; i < 120; ++i) {
        rp::InputState input;
        world.step(world.materials().fixedDt, input, 1280.0, 720.0);
    }

    const bool anyInvalid = std::any_of(world.points().begin(), world.points().end(), [](const rp::Point& point) {
        return !std::isfinite(point.position.x) || !std::isfinite(point.position.y);
    });
    if (anyInvalid) {
        return fail("rest simulation produced an invalid coordinate");
    }
    if (world.stats().brokenSkin != 0 ||
        world.stats().brokenMuscle != 0 ||
        world.stats().brokenAttachments != 0 ||
        world.stats().brokenBoneAttachments != 0 ||
        world.stats().brokenBoneJoints != 0 ||
        world.stats().emittedFluidParticles != 0 ||
        world.stats().openedWounds != 0 ||
        world.stats().woundFluidParticles != 0 ||
        world.stats().fragmentTissueHits != 0 ||
        world.stats().fragmentTissueTears != 0 ||
        world.stats().fracturedBones != 0) {
        return fail("rest simulation should not tear tissue");
    }
    if (!world.fluids().empty() || !world.wounds().empty()) {
        return fail("rest simulation should not emit fluid particles");
    }
    if (world.debug().active || world.debug().impact != 0.0 || world.debug().boneContacts != 0 || world.debug().tissueContacts != 0) {
        return fail("inactive input should leave contact debug metrics idle");
    }
    rp::InputState inactiveSharp;
    inactiveSharp.tool = rp::ToolMode::Sharp;
    world.step(world.materials().fixedDt, inactiveSharp, 1280.0, 720.0);
    if (world.debug().tool != rp::ToolMode::Sharp || world.debug().active || world.debug().impact != 0.0) {
        return fail("inactive input should report selected tool without creating impact");
    }

    rp::World directBoneWorld = rp::createLayeredBody(1280.0, 720.0);
    if (directBoneWorld.bones().size() < 2) {
        return fail("direct bone strike scenario needs a torso bone");
    }
    const rp::BoneSegment directTarget = directBoneWorld.bones()[1];
    const rp::Vec2 directCenter{
        (directTarget.a.x + directTarget.b.x) * 0.5,
        (directTarget.a.y + directTarget.b.y) * 0.5,
    };
    const std::size_t directInitialBoneCount = directBoneWorld.bones().size();

    rp::InputState directStrike;
    directStrike.active = true;
    directStrike.down = true;
    directStrike.x = directCenter.x;
    directStrike.y = directCenter.y;
    directStrike.vx = 2200.0;
    directStrike.vy = 120.0;
    directStrike.power = 4.0;
    directBoneWorld.step(directBoneWorld.materials().fixedDt, directStrike, 1280.0, 720.0);

    const rp::BoneSegment movedTarget = directBoneWorld.bones()[1];
    const double directMovement = std::max(rp::distance(directTarget.a, movedTarget.a), rp::distance(directTarget.b, movedTarget.b));
    if (directMovement < 2.0) {
        return fail("direct striker contact should move bone endpoints");
    }
    if (directBoneWorld.stats().fracturedBones <= 0 || directBoneWorld.bones().size() <= directInitialBoneCount) {
        return fail("direct striker contact should fracture a bone");
    }
    if (directBoneWorld.stats().brokenMuscle <= 0 || directBoneWorld.stats().brokenBoneAttachments <= 0) {
        return fail("bone fracture should damage and release nearby tissue");
    }
    if (directBoneWorld.stats().emittedFluidParticles <= 0 || directBoneWorld.fluids().empty()) {
        return fail("bone fracture should emit fluid particles from damaged tissue");
    }
    if (directBoneWorld.stats().fragmentTissueHits <= 0 || directBoneWorld.debug().fragmentContacts <= 0) {
        return fail("broken bone ends should collide with nearby tissue after fracture");
    }
    if (!directBoneWorld.debug().down ||
        directBoneWorld.debug().tool != rp::ToolMode::Blunt ||
        directBoneWorld.debug().impact <= 0.0 ||
        directBoneWorld.debug().boneContacts <= 0 ||
        directBoneWorld.debug().maxBoneLoad < directTarget.fractureImpulse ||
        directBoneWorld.debug().maxFragmentImpulse <= 0.0 ||
        directBoneWorld.debug().fractures <= 0 ||
        directBoneWorld.debug().fluidEmitted <= 0) {
        return fail("direct strike should expose contact debug metrics");
    }
    if (directBoneWorld.stats().openedWounds <= 0 || activeWoundCount(directBoneWorld) <= 0) {
        return fail("bone fracture should open persistent wound sources");
    }
    const int directBurstFluid = directBoneWorld.stats().emittedFluidParticles;
    for (int i = 0; i < 30; ++i) {
        rp::InputState noInput;
        directBoneWorld.step(directBoneWorld.materials().fixedDt, noInput, 1280.0, 720.0);
    }
    if (directBoneWorld.stats().woundFluidParticles <= 0 ||
        directBoneWorld.stats().emittedFluidParticles <= directBurstFluid ||
        directBoneWorld.debug().maxWoundPressure <= 0.0) {
        return fail("open fracture wounds should keep leaking after impact");
    }

    rp::World sharpWorld;
    sharpWorld.addPoint({130.0, 120.0}, rp::TissueLayer::Skin, false);
    sharpWorld.addPoint({170.0, 120.0}, rp::TissueLayer::Skin, false);
    sharpWorld.addSpring(0, 1, rp::TissueLayer::Skin, 0.82, 10.0, 1000.0);
    rp::InputState sharpStrike;
    sharpStrike.active = true;
    sharpStrike.down = true;
    sharpStrike.x = 150.0;
    sharpStrike.y = 120.0;
    sharpStrike.vx = 900.0;
    sharpStrike.vy = 0.0;
    sharpStrike.power = 3.0;
    sharpStrike.tool = rp::ToolMode::Sharp;
    sharpWorld.step(sharpWorld.materials().fixedDt, sharpStrike, 640.0, 480.0);
    if (sharpWorld.debug().tool != rp::ToolMode::Sharp ||
        sharpWorld.debug().strikerRadius >= directBoneWorld.materials().strikerRadius ||
        sharpWorld.stats().brokenSkin <= 0) {
        return fail("sharp tool should concentrate pressure into skin tearing");
    }
    if (sharpWorld.stats().openedWounds <= 0 || activeWoundCount(sharpWorld) <= 0) {
        return fail("sharp skin tears should open a persistent wound source");
    }

    rp::World sharpNormalWorld;
    sharpNormalWorld.addPoint({130.0, 120.0}, rp::TissueLayer::Skin, false);
    sharpNormalWorld.addPoint({170.0, 120.0}, rp::TissueLayer::Skin, false);
    sharpNormalWorld.addSpring(0, 1, rp::TissueLayer::Skin, 0.82, 10.0, 1000.0);
    rp::InputState verticalCut = sharpStrike;
    verticalCut.vx = 0.0;
    verticalCut.vy = 900.0;
    sharpNormalWorld.step(sharpNormalWorld.materials().fixedDt, verticalCut, 640.0, 480.0);
    if (sharpNormalWorld.wounds().empty() ||
        std::abs(sharpNormalWorld.wounds().front().direction.x) <=
            std::abs(sharpNormalWorld.wounds().front().direction.y) * 2.5) {
        return fail("sharp wound direction should follow blade motion instead of only spring orientation");
    }

    rp::World sharpTipWorld;
    sharpTipWorld.addPoint({165.0, 120.0}, rp::TissueLayer::Skin, false);
    sharpTipWorld.addPoint({185.0, 120.0}, rp::TissueLayer::Skin, false);
    sharpTipWorld.addSpring(0, 1, rp::TissueLayer::Skin, 0.82, 10.0, 1000.0);
    sharpTipWorld.step(sharpTipWorld.materials().fixedDt, sharpStrike, 640.0, 480.0);
    if (sharpTipWorld.stats().brokenSkin <= 0 || sharpTipWorld.debug().tissueContacts <= 0) {
        return fail("sharp tool should contact and cut along the rendered blade segment, not just at the handle center");
    }

    for (int i = 0; i < 24; ++i) {
        rp::InputState noInput;
        sharpWorld.step(sharpWorld.materials().fixedDt, noInput, 640.0, 480.0);
    }
    if (sharpWorld.stats().woundFluidParticles <= 0 || sharpWorld.debug().maxWoundClot <= 0.0) {
        return fail("skin wound pressure should leak and begin clotting over time");
    }

    rp::World bluntLoadWorld;
    rp::World heavyLoadWorld;
    bluntLoadWorld.addBoneSegment({100.0, 180.0}, {220.0, 180.0}, 7.0, 999999.0);
    heavyLoadWorld.addBoneSegment({100.0, 180.0}, {220.0, 180.0}, 7.0, 999999.0);
    rp::InputState loadStrike;
    loadStrike.active = true;
    loadStrike.down = true;
    loadStrike.x = 160.0;
    loadStrike.y = 180.0;
    loadStrike.vx = 900.0;
    loadStrike.vy = 0.0;
    loadStrike.power = 2.0;
    bluntLoadWorld.step(bluntLoadWorld.materials().fixedDt, loadStrike, 640.0, 480.0);
    loadStrike.tool = rp::ToolMode::Heavy;
    heavyLoadWorld.step(heavyLoadWorld.materials().fixedDt, loadStrike, 640.0, 480.0);
    if (heavyLoadWorld.debug().tool != rp::ToolMode::Heavy ||
        heavyLoadWorld.debug().strikerMass <= bluntLoadWorld.debug().strikerMass ||
        heavyLoadWorld.debug().maxBoneLoad <= bluntLoadWorld.debug().maxBoneLoad * 1.35) {
        return fail("heavy tool should apply a larger mass-scaled bone load than blunt");
    }

    rp::Materials transferMaterials;
    transferMaterials.boneJointBreakStretch = 99.0;
    transferMaterials.boneJointBreakImpulse = 999999.0;
    transferMaterials.boneJointAngularBreak = 99.0;
    rp::World jointWorld(transferMaterials);
    const std::size_t jointA = jointWorld.addBoneSegment({100.0, 120.0}, {200.0, 120.0}, 6.0, 999999.0);
    const std::size_t jointB = jointWorld.addBoneSegment({205.0, 120.0}, {305.0, 120.0}, 6.0, 999999.0);
    jointWorld.addBoneJoint(jointA, 1.0, jointB, 0.0);
    const rp::BoneSegment beforeJointB = jointWorld.bones()[jointB];
    rp::InputState jointStrike;
    jointStrike.active = true;
    jointStrike.down = true;
    jointStrike.x = 160.0;
    jointStrike.y = 120.0;
    jointStrike.vx = 260.0;
    jointStrike.vy = 0.0;
    jointStrike.power = 0.6;
    jointWorld.step(jointWorld.materials().fixedDt, jointStrike, 640.0, 480.0);
    const rp::BoneSegment afterJointB = jointWorld.bones()[jointB];
    const double jointTransfer = std::max(rp::distance(beforeJointB.a, afterJointB.a), rp::distance(beforeJointB.b, afterJointB.b));
    if (jointTransfer < 0.35 || jointWorld.stats().brokenBoneJoints != 0) {
        return fail("bone joints should transfer motion without breaking under moderate load");
    }

    rp::Materials angularMaterials;
    angularMaterials.boneJointAngularBreak = 0.06;
    angularMaterials.boneJointBreakStretch = 99.0;
    angularMaterials.boneJointBreakImpulse = 999999.0;
    angularMaterials.solverIterations = 4;
    rp::World angularWorld(angularMaterials);
    const std::size_t angularA = angularWorld.addBoneSegment({100.0, 180.0}, {200.0, 180.0}, 6.0, 999999.0, true);
    const std::size_t angularB = angularWorld.addBoneSegment({205.0, 180.0}, {305.0, 180.0}, 6.0, 999999.0);
    angularWorld.addBoneJoint(angularA, 1.0, angularB, 0.0, -0.04, 0.04);
    for (int i = 0; i < 8 && angularWorld.stats().brokenBoneJoints == 0; ++i) {
        rp::InputState overextensionStrike;
        overextensionStrike.active = true;
        overextensionStrike.down = true;
        overextensionStrike.x = 305.0;
        overextensionStrike.y = 180.0;
        overextensionStrike.vx = 0.0;
        overextensionStrike.vy = 2200.0;
        overextensionStrike.power = 4.0;
        angularWorld.step(angularWorld.materials().fixedDt, overextensionStrike, 640.0, 480.0);
    }
    if (angularWorld.stats().brokenBoneJoints <= 0) {
        return fail("bone joints should break when a connected bone is overextended");
    }
    for (int i = 0; i < 48; ++i) {
        rp::InputState noInput;
        angularWorld.step(angularWorld.materials().fixedDt, noInput, 640.0, 480.0);
    }
    const rp::BoneJoint& brokenJoint = angularWorld.boneJoints()[0];
    const double brokenJointLimit = std::max(brokenJoint.rest + angularWorld.materials().postFractureJointSlack,
                                             brokenJoint.rest * angularWorld.materials().postFractureJointMaxStretch);
    if (jointAnchorDistance(angularWorld, brokenJoint) > brokenJointLimit + 12.0 ||
        jointAngleViolation(angularWorld, brokenJoint) > 0.35) {
        return fail("broken bone joints should still limit impossible post-fracture stretch and twist");
    }

    rp::Materials partialJointMaterials;
    partialJointMaterials.maxBoneFractureDepth = 1;
    partialJointMaterials.boneJointBreakStretch = 99.0;
    partialJointMaterials.boneJointBreakImpulse = 999999.0;
    partialJointMaterials.boneJointAngularBreak = 99.0;
    partialJointMaterials.postFractureJointStiffness = 0.20;
    partialJointMaterials.postFractureJointAngularStiffness = 0.08;
    rp::World partialJointWorld(partialJointMaterials);
    const std::size_t partialAnchor = partialJointWorld.addBoneSegment({100.0, 180.0}, {180.0, 180.0}, 7.0, 999999.0, true);
    const std::size_t partialLimb = partialJointWorld.addBoneSegment({185.0, 180.0}, {385.0, 180.0}, 7.0, 850.0);
    partialJointWorld.addBoneJoint(partialAnchor, 1.0, partialLimb, 0.0, -0.35, 0.35);
    rp::InputState partialStrike;
    partialStrike.active = true;
    partialStrike.down = true;
    partialStrike.x = 255.0;
    partialStrike.y = 180.0;
    partialStrike.vx = 1500.0;
    partialStrike.vy = 120.0;
    partialStrike.power = 4.0;
    partialJointWorld.step(partialJointWorld.materials().fixedDt, partialStrike, 640.0, 480.0);
    if (partialJointWorld.stats().fracturedBones <= 0 || partialJointWorld.boneJoints().empty()) {
        return fail("jointed limb strike should fracture a connected bone");
    }
    if (!partialJointWorld.boneJoints()[0].postFractureLimited || partialJointWorld.boneJoints()[0].broken) {
        return fail("surviving remapped bone joints should become post-fracture limited");
    }
    const double partialTipY = partialJointWorld.bones()[partialLimb].b.y;
    for (int i = 0; i < 60; ++i) {
        rp::InputState noInput;
        partialJointWorld.step(partialJointWorld.materials().fixedDt, noInput, 640.0, 480.0);
    }
    const rp::BoneJoint& partialJoint = partialJointWorld.boneJoints()[0];
    const double partialLimit = std::max(partialJoint.postFractureRest + partialJointWorld.materials().postFractureJointSlack,
                                         partialJoint.postFractureRest * partialJointWorld.materials().postFractureJointMaxStretch);
    if (partialJointWorld.debug().maxPostFractureJointStretch <= 0.0 ||
        partialJointWorld.bones()[partialLimb].b.y <= partialTipY + 3.0) {
        return fail("post-fracture limited joints should allow a partially attached limb to sag");
    }
    if (jointAnchorDistance(partialJointWorld, partialJoint) > partialLimit + 10.0 ||
        jointAngleViolation(partialJointWorld, partialJoint) > 0.30) {
        return fail("post-fracture limited joints should bound partially attached limb stretch and twist");
    }

    rp::Materials offCenterMaterials;
    offCenterMaterials.maxBoneFractureDepth = 1;
    rp::World offCenterWorld(offCenterMaterials);
    const std::size_t offCenterBone = offCenterWorld.addBoneSegment({100.0, 180.0}, {300.0, 180.0}, 7.0, 1000.0);
    rp::InputState offCenterStrike;
    offCenterStrike.active = true;
    offCenterStrike.down = true;
    offCenterStrike.x = 146.0;
    offCenterStrike.y = 180.0;
    offCenterStrike.vx = 1300.0;
    offCenterStrike.vy = 0.0;
    offCenterStrike.power = 4.0;
    offCenterWorld.step(offCenterWorld.materials().fixedDt, offCenterStrike, 640.0, 480.0);
    if (offCenterWorld.stats().fracturedBones < 1 || offCenterWorld.bones().size() < 3) {
        return fail("off-center contact should create separated fracture fragments and a splinter");
    }
    const rp::BoneSegment& offCenterFirst = offCenterWorld.bones()[offCenterBone];
    const rp::BoneSegment& offCenterSecond = offCenterWorld.bones()[offCenterBone + 1];
    if (!offCenterFirst.brokenEnd || !offCenterSecond.brokenStart) {
        return fail("off-center fracture should mark matching broken ends");
    }
    const double shortPiece = rp::distance(offCenterFirst.a, offCenterFirst.b);
    const double longPiece = rp::distance(offCenterSecond.a, offCenterSecond.b);
    if (shortPiece >= longPiece * 0.62) {
        return fail("off-center fracture should split near the contact point, not at the midpoint");
    }
    double fractureGap = rp::distance(offCenterFirst.b, offCenterSecond.a);
    if (fractureGap < 10.0) {
        return fail("fractured bone ends should open a visible gap");
    }
    const double offCenterAngularSpeed = offCenterWorld.debug().maxBoneAngularSpeed;
    const double fragmentAngleBeforeSettle = testBoneAngle(offCenterSecond);
    if (offCenterAngularSpeed <= 0.05) {
        return fail("off-center fracture should seed fragment angular velocity");
    }
    for (int i = 0; i < 24; ++i) {
        rp::InputState noInput;
        offCenterWorld.step(offCenterWorld.materials().fixedDt, noInput, 640.0, 480.0);
    }
    fractureGap = rp::distance(offCenterWorld.bones()[offCenterBone].b, offCenterWorld.bones()[offCenterBone + 1].a);
    if (fractureGap < 7.0) {
        return fail("fracture gap should not immediately collapse back into alignment");
    }
    const double fragmentAngleAfterSettle = testBoneAngle(offCenterWorld.bones()[offCenterBone + 1]);
    if (angleDelta(fragmentAngleBeforeSettle, fragmentAngleAfterSettle) < 0.02) {
        return fail("free fractured fragments should keep rotating while they settle");
    }
    if (maxFreeFragmentOverlap(offCenterWorld) > 2.0) {
        return fail("off-center fractured fragments should not settle while deeply overlapping");
    }

    rp::Materials fragmentSeparationMaterials;
    fragmentSeparationMaterials.maxBoneFractureDepth = 1;
    fragmentSeparationMaterials.solverIterations = 12;
    rp::World fragmentSeparationWorld(fragmentSeparationMaterials);
    fragmentSeparationWorld.addBoneSegment({100.0, 180.0}, {320.0, 180.0}, 8.0, 800.0);
    fragmentSeparationWorld.addBoneSegment({105.0, 180.0}, {325.0, 180.0}, 8.0, 800.0);
    rp::InputState fragmentSeparationStrike;
    fragmentSeparationStrike.active = true;
    fragmentSeparationStrike.down = true;
    fragmentSeparationStrike.x = 210.0;
    fragmentSeparationStrike.y = 180.0;
    fragmentSeparationStrike.vx = 1600.0;
    fragmentSeparationStrike.vy = 0.0;
    fragmentSeparationStrike.power = 4.0;
    fragmentSeparationWorld.step(fragmentSeparationWorld.materials().fixedDt, fragmentSeparationStrike, 640.0, 480.0);
    if (fragmentSeparationWorld.debug().fragmentPairContacts <= 0 ||
        fragmentSeparationWorld.debug().maxFragmentOverlap <= 0.0) {
        return fail("overlapping fractured bones should report fragment-to-fragment contacts");
    }
    for (int i = 0; i < 12; ++i) {
        rp::InputState noInput;
        fragmentSeparationWorld.step(fragmentSeparationWorld.materials().fixedDt, noInput, 640.0, 480.0);
    }
    if (maxFreeFragmentOverlap(fragmentSeparationWorld) > 2.5) {
        return fail("fragment repulsion should push overlapping broken bones apart");
    }

    rp::World refractureWorld;
    const std::size_t refractureBone = refractureWorld.addBoneSegment({100.0, 180.0}, {320.0, 180.0}, 7.0, 1000.0);
    const std::size_t firstFragmentCount = refractureWorld.bones().size();
    const rp::BoneSegment refractureTarget = refractureWorld.bones()[refractureBone];
    rp::InputState refractureStrike;
    refractureStrike.active = true;
    refractureStrike.down = true;
    refractureStrike.x = (refractureTarget.a.x + refractureTarget.b.x) * 0.5;
    refractureStrike.y = (refractureTarget.a.y + refractureTarget.b.y) * 0.5;
    refractureStrike.vx = -1600.0;
    refractureStrike.vy = 320.0;
    refractureStrike.power = 4.0;
    refractureWorld.step(refractureWorld.materials().fixedDt, refractureStrike, 640.0, 480.0);
    if (refractureWorld.stats().fracturedBones < 2 || refractureWorld.bones().size() <= firstFragmentCount + 2) {
        return fail("large broken fragments should be able to keep fracturing under high load");
    }

    rp::InputState strike;
    strike.active = true;
    strike.down = true;
    strike.x = 670.0;
    strike.y = 360.0;
    strike.vx = 1800.0;
    strike.vy = 80.0;
    strike.power = 4.0;

    for (int i = 0; i < 45; ++i) {
        strike.x += 5.5;
        world.step(world.materials().fixedDt, strike, 1280.0, 720.0);
    }

    if (world.stats().brokenSkin <= 0) {
        return fail("high-energy strike should tear skin");
    }
    if (world.stats().fracturedBones <= 0) {
        return fail("high-energy strike should fracture bone segments");
    }
    if (world.bones().size() <= initialBoneCount) {
        return fail("fractured bone should split into independent segments");
    }
    const bool hasVisibleBrokenEnd = std::any_of(world.bones().begin(), world.bones().end(), [](const rp::BoneSegment& bone) {
        return bone.brokenStart || bone.brokenEnd;
    });
    if (!hasVisibleBrokenEnd) {
        return fail("fractured bones should expose broken ends");
    }
    if (world.stats().emittedFluidParticles <= 0) {
        return fail("high-energy strike should emit fluid from torn tissue");
    }
    const bool invalidFluid = std::any_of(world.fluids().begin(), world.fluids().end(), [](const rp::FluidParticle& fluid) {
        return fluid.life > 0.0 && (!std::isfinite(fluid.position.x) || !std::isfinite(fluid.position.y));
    });
    if (invalidFluid) {
        return fail("fluid simulation produced an invalid coordinate");
    }

    const int liveSkinTriangles = static_cast<int>(std::count_if(world.triangles().begin(), world.triangles().end(), [&](const rp::Triangle& triangle) {
        return triangle.layer == rp::TissueLayer::Skin && world.triangleAlive(triangle);
    }));
    if (liveSkinTriangles >= initialLiveSkinTriangles) {
        return fail("high-energy strike should open at least one skin triangle");
    }

    std::cout << "PASS: points=" << world.points().size()
              << " springs=" << world.springs().size()
              << " triangles=" << world.triangles().size()
              << " bones=" << world.bones().size()
              << " skin_tears=" << world.stats().brokenSkin
              << " muscle_tears=" << world.stats().brokenMuscle
              << " detachments=" << world.stats().brokenAttachments
              << " bone_detachments=" << world.stats().brokenBoneAttachments
              << " bone_joints=" << world.boneJoints().size()
              << " broken_bone_joints=" << world.stats().brokenBoneJoints
              << " bone_fractures=" << world.stats().fracturedBones
              << " fluid_particles=" << world.stats().emittedFluidParticles
              << " wounds=" << world.stats().openedWounds
              << " wound_fluid=" << world.stats().woundFluidParticles
              << " fragment_hits=" << world.stats().fragmentTissueHits
              << " fragment_tears=" << world.stats().fragmentTissueTears
              << '\n';
    return 0;
}
