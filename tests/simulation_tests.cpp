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
        world.stats().fracturedBones != 0) {
        return fail("rest simulation should not tear tissue");
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
              << " bone_fractures=" << world.stats().fracturedBones
              << '\n';
    return 0;
}
