#include "simulation.hpp"

#include <algorithm>
#include <cmath>
#include <iostream>

namespace {

int fail(const char* message) {
    std::cerr << "FAIL: " << message << '\n';
    return 1;
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
        world.stats().fragmentTissueHits != 0 ||
        world.stats().fragmentTissueTears != 0 ||
        world.stats().fracturedBones != 0) {
        return fail("rest simulation should not tear tissue");
    }
    if (!world.fluids().empty()) {
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

    rp::World jointWorld;
    const std::size_t jointA = jointWorld.addBoneSegment({100.0, 120.0}, {200.0, 120.0}, 6.0, 999999.0);
    const std::size_t jointB = jointWorld.addBoneSegment({205.0, 120.0}, {305.0, 120.0}, 6.0, 999999.0);
    jointWorld.addBoneJoint(jointA, 1.0, jointB, 0.0);
    const rp::BoneSegment beforeJointB = jointWorld.bones()[jointB];
    rp::InputState jointStrike;
    jointStrike.active = true;
    jointStrike.down = true;
    jointStrike.x = 195.0;
    jointStrike.y = 120.0;
    jointStrike.vx = 1200.0;
    jointStrike.vy = 0.0;
    jointStrike.power = 2.0;
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

    rp::World offCenterWorld;
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
    if (offCenterWorld.stats().fracturedBones != 1 || offCenterWorld.bones().size() < 3) {
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
    for (int i = 0; i < 24; ++i) {
        rp::InputState noInput;
        offCenterWorld.step(offCenterWorld.materials().fixedDt, noInput, 640.0, 480.0);
    }
    fractureGap = rp::distance(offCenterWorld.bones()[offCenterBone].b, offCenterWorld.bones()[offCenterBone + 1].a);
    if (fractureGap < 7.0) {
        return fail("fracture gap should not immediately collapse back into alignment");
    }

    rp::World fragmentWorld;
    fragmentWorld.addPoint({180.0, 170.0}, rp::TissueLayer::Muscle, false);
    fragmentWorld.addPoint({218.0, 170.0}, rp::TissueLayer::Muscle, false);
    fragmentWorld.addSpring(0, 1, rp::TissueLayer::Muscle, 0.74, 10.0, 720.0);
    fragmentWorld.addBoneSegment({100.0, 170.0}, {300.0, 170.0}, 7.0, 800.0);
    rp::InputState fragmentStrike;
    fragmentStrike.active = true;
    fragmentStrike.down = true;
    fragmentStrike.x = 178.0;
    fragmentStrike.y = 170.0;
    fragmentStrike.vx = 1350.0;
    fragmentStrike.vy = 0.0;
    fragmentStrike.power = 4.0;
    fragmentWorld.step(fragmentWorld.materials().fixedDt, fragmentStrike, 640.0, 480.0);
    if (fragmentWorld.stats().fracturedBones <= 0 ||
        fragmentWorld.stats().fragmentTissueHits <= 0 ||
        fragmentWorld.debug().maxFragmentDepth <= 0.0) {
        return fail("broken bone fragments should collide with nearby muscle after fracture");
    }

    const std::size_t firstFragmentCount = offCenterWorld.bones().size();
    const rp::BoneSegment refractureTarget = offCenterWorld.bones()[offCenterBone + 1];
    rp::InputState refractureStrike;
    refractureStrike.active = true;
    refractureStrike.down = true;
    refractureStrike.x = (refractureTarget.a.x + refractureTarget.b.x) * 0.5;
    refractureStrike.y = (refractureTarget.a.y + refractureTarget.b.y) * 0.5;
    refractureStrike.vx = -1600.0;
    refractureStrike.vy = 320.0;
    refractureStrike.power = 4.0;
    offCenterWorld.step(offCenterWorld.materials().fixedDt, refractureStrike, 640.0, 480.0);
    if (offCenterWorld.stats().fracturedBones < 2 || offCenterWorld.bones().size() <= firstFragmentCount) {
        return fail("long fractured fragments should be able to fracture again");
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
              << " fragment_hits=" << world.stats().fragmentTissueHits
              << " fragment_tears=" << world.stats().fragmentTissueTears
              << '\n';
    return 0;
}
