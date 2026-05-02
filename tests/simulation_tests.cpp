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
    if (world.stats().brokenSkin != 0 || world.stats().brokenMuscle != 0 || world.stats().brokenAttachments != 0) {
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

    std::cout << "PASS: points=" << world.points().size()
              << " springs=" << world.springs().size()
              << " triangles=" << world.triangles().size()
              << " skin_tears=" << world.stats().brokenSkin
              << " muscle_tears=" << world.stats().brokenMuscle
              << " detachments=" << world.stats().brokenAttachments
              << '\n';
    return 0;
}
