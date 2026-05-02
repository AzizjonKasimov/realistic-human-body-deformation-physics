#include "simulation.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <string>
#include <unordered_map>

namespace rp {
namespace {

constexpr double kEpsilon = 0.0001;

struct GridKey {
    int x = 0;
    int y = 0;

    bool operator==(const GridKey& other) const {
        return x == other.x && y == other.y;
    }
};

struct GridKeyHash {
    std::size_t operator()(const GridKey& key) const {
        const std::uint64_t ux = static_cast<std::uint32_t>(key.x);
        const std::uint64_t uy = static_cast<std::uint32_t>(key.y);
        return static_cast<std::size_t>((ux << 32U) ^ uy);
    }
};

bool ellipse(double x, double y, double cx, double cy, double rx, double ry) {
    const double dx = (x - cx) / rx;
    const double dy = (y - cy) / ry;
    return dx * dx + dy * dy <= 1.0;
}

bool box(double x, double y, double minX, double maxX, double minY, double maxY) {
    return x >= minX && x <= maxX && y >= minY && y <= maxY;
}

bool capsule(double x, double y, double ax, double ay, double bx, double by, double radius) {
    const double abx = bx - ax;
    const double aby = by - ay;
    const double apx = x - ax;
    const double apy = y - ay;
    const double abLenSq = abx * abx + aby * aby;
    const double t = std::clamp((apx * abx + apy * aby) / abLenSq, 0.0, 1.0);
    const double cx = ax + abx * t;
    const double cy = ay + aby * t;
    const double dx = x - cx;
    const double dy = y - cy;
    return dx * dx + dy * dy <= radius * radius;
}

bool isInsideHumanoid(double nx, double ny) {
    const bool head = ellipse(nx, ny, 0.0, 0.105, 0.078, 0.085);
    const bool neck = box(nx, ny, -0.034, 0.034, 0.17, 0.25);
    const bool shoulders = ellipse(nx, ny, 0.0, 0.275, 0.205, 0.075);
    const bool chest = ellipse(nx, ny, 0.0, 0.43, 0.155, 0.225);
    const bool hips = ellipse(nx, ny, 0.0, 0.64, 0.132, 0.11);
    const bool leftArm = capsule(nx, ny, -0.195, 0.285, -0.245, 0.62, 0.052);
    const bool rightArm = capsule(nx, ny, 0.195, 0.285, 0.245, 0.62, 0.052);
    const bool leftLeg = capsule(nx, ny, -0.065, 0.675, -0.082, 0.97, 0.056);
    const bool rightLeg = capsule(nx, ny, 0.065, 0.675, 0.082, 0.97, 0.056);
    return head || neck || shoulders || chest || hips || leftArm || rightArm || leftLeg || rightLeg;
}

void applyPairCorrection(Point& a, Point& b, double correctionX, double correctionY) {
    const double invMassA = a.pinned ? 0.0 : 1.0 / a.mass;
    const double invMassB = b.pinned ? 0.0 : 1.0 / b.mass;
    const double sum = invMassA + invMassB;
    if (sum <= 0.0) {
        return;
    }

    if (!a.pinned) {
        a.position.x += correctionX * (invMassA / sum);
        a.position.y += correctionY * (invMassA / sum);
    }
    if (!b.pinned) {
        b.position.x -= correctionX * (invMassB / sum);
        b.position.y -= correctionY * (invMassB / sum);
    }
}

void addCellTriangles(World& world,
                      const std::unordered_map<GridKey, std::size_t, GridKeyHash>& grid,
                      int x,
                      int y,
                      TissueLayer layer,
                      double areaStiffness) {
    const auto a = grid.find({x, y});
    const auto b = grid.find({x + 1, y});
    const auto c = grid.find({x, y + 1});
    const auto d = grid.find({x + 1, y + 1});

    if (a != grid.end() && b != grid.end() && c != grid.end()) {
        world.addTriangle(a->second, b->second, c->second, layer);
        world.addArea(a->second, b->second, c->second, layer, areaStiffness);
    }
    if (b != grid.end() && d != grid.end() && c != grid.end()) {
        world.addTriangle(b->second, d->second, c->second, layer);
        world.addArea(b->second, d->second, c->second, layer, areaStiffness);
    }
}

} // namespace

double distance(Vec2 a, Vec2 b) {
    const double dx = b.x - a.x;
    const double dy = b.y - a.y;
    return std::sqrt(dx * dx + dy * dy);
}

double signedArea(Vec2 a, Vec2 b, Vec2 c) {
    return ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) * 0.5;
}

World::World(Materials materials)
    : materials_(materials) {}

std::size_t World::addPoint(Vec2 position, TissueLayer layer, bool pinned) {
    const std::size_t index = points_.size();
    Point point;
    point.position = position;
    point.previous = position;
    point.home = position;
    point.layer = layer;
    point.pinned = pinned;
    point.mass = layer == TissueLayer::Muscle ? 1.25 : 1.0;
    points_.push_back(point);
    return index;
}

void World::addSpring(std::size_t a, std::size_t b, TissueLayer layer, double stiffness, double tearStretch, double tearImpulse, bool fiber) {
    if (a == b || a >= points_.size() || b >= points_.size()) {
        return;
    }

    for (const Spring& spring : springs_) {
        const bool sameLayer = spring.layer == layer;
        const bool sameEdge = (spring.a == a && spring.b == b) || (spring.a == b && spring.b == a);
        if (sameLayer && sameEdge) {
            return;
        }
    }

    Spring spring;
    spring.a = a;
    spring.b = b;
    spring.rest = distance(points_[a].position, points_[b].position);
    spring.stiffness = stiffness;
    spring.tearStretch = tearStretch;
    spring.tearImpulse = tearImpulse;
    spring.layer = layer;
    spring.fiber = fiber;
    springs_.push_back(spring);
}

void World::addArea(std::size_t a, std::size_t b, std::size_t c, TissueLayer layer, double stiffness) {
    AreaConstraint area;
    area.a = a;
    area.b = b;
    area.c = c;
    area.layer = layer;
    area.restArea = signedArea(points_[a].position, points_[b].position, points_[c].position);
    area.stiffness = stiffness;
    areas_.push_back(area);
}

void World::addAttachment(std::size_t skinPoint, std::size_t musclePoint) {
    Attachment attachment;
    attachment.skinPoint = skinPoint;
    attachment.musclePoint = musclePoint;
    attachment.rest = distance(points_[skinPoint].position, points_[musclePoint].position);
    attachments_.push_back(attachment);
}

void World::addTriangle(std::size_t a, std::size_t b, std::size_t c, TissueLayer layer) {
    Triangle triangle;
    triangle.a = a;
    triangle.b = b;
    triangle.c = c;
    triangle.layer = layer;
    triangles_.push_back(triangle);
}

void World::step(double dt, const InputState& input, double width, double height) {
    const double floorY = height - 38.0;
    updateExposure();
    integrate(dt, width, floorY);
    collideStriker(dt, input);

    for (int i = 0; i < materials_.solverIterations; ++i) {
        solveSprings();
        solveAttachments();
        solveAreas();
        constrainToWorld(width, floorY);
    }

    updateTriangleDamage();
}

bool World::triangleAlive(const Triangle& triangle) const {
    if (triangle.failed) {
        return false;
    }

    const int liveEdges =
        (hasLiveSpring(triangle.a, triangle.b, triangle.layer) ? 1 : 0) +
        (hasLiveSpring(triangle.b, triangle.c, triangle.layer) ? 1 : 0) +
        (hasLiveSpring(triangle.c, triangle.a, triangle.layer) ? 1 : 0);
    return liveEdges >= 2;
}

bool World::hasLiveSpring(std::size_t a, std::size_t b, TissueLayer layer) const {
    for (const Spring& spring : springs_) {
        if (spring.layer != layer || spring.broken) {
            continue;
        }
        if ((spring.a == a && spring.b == b) || (spring.a == b && spring.b == a)) {
            return true;
        }
    }
    return false;
}

void World::integrate(double dt, double width, double floorY) {
    (void)width;
    for (Point& point : points_) {
        point.load *= 0.84;
        point.exposure *= 0.92;

        if (point.pinned) {
            point.position = point.home;
            point.previous = point.position;
            continue;
        }

        const double vx = (point.position.x - point.previous.x) * materials_.damping;
        const double vy = (point.position.y - point.previous.y) * materials_.damping;
        point.previous = point.position;
        point.position.x += vx;
        point.position.y += vy + materials_.gravity * dt * dt;

        const double shapeStiffness = point.layer == TissueLayer::Skin ? materials_.skinShapeStiffness : materials_.muscleShapeStiffness;
        point.position.x += (point.home.x - point.position.x) * shapeStiffness;
        point.position.y += (point.home.y - point.position.y) * shapeStiffness;

        if (point.position.y > floorY) {
            point.position.y = floorY;
            point.previous.x = point.position.x + (point.previous.x - point.position.x) * materials_.floorFriction;
        }
    }
}

void World::collideStriker(double dt, const InputState& input) {
    if (!input.active) {
        return;
    }

    const double speed = std::sqrt(input.vx * input.vx + input.vy * input.vy);
    const double impact = speed * materials_.strikerMass * input.power;
    const double influence = materials_.strikerRadius + 12.0;

    for (Point& point : points_) {
        if (point.pinned) {
            continue;
        }

        const double dx = point.position.x - input.x;
        const double dy = point.position.y - input.y;
        const double dist = std::sqrt(dx * dx + dy * dy);
        if (dist > influence || dist < kEpsilon) {
            continue;
        }

        double contactStrength = (input.down ? 0.74 : 0.20) * (0.85 + input.power * 0.15);
        if (point.layer == TissueLayer::Muscle) {
            contactStrength *= materials_.directMuscleContact + point.exposure * 0.82;
        }

        const double nx = dx / dist;
        const double ny = dy / dist;
        const double depth = influence - dist;
        point.position.x += nx * depth * contactStrength + input.vx * dt * 0.45 * contactStrength;
        point.position.y += ny * depth * contactStrength + input.vy * dt * 0.45 * contactStrength;
        point.load = std::max(point.load, impact * (depth / influence) * contactStrength);
    }
}

void World::solveSprings() {
    for (Spring& spring : springs_) {
        if (spring.broken) {
            continue;
        }

        Point& a = points_[spring.a];
        Point& b = points_[spring.b];
        const double dx = b.position.x - a.position.x;
        const double dy = b.position.y - a.position.y;
        const double len = std::sqrt(dx * dx + dy * dy);
        if (len < kEpsilon) {
            continue;
        }

        const double stretchRatio = len / spring.rest;
        const double endpointLoad = std::max(a.load, b.load);
        const double tearImpulse = spring.layer == TissueLayer::Muscle
            ? spring.tearImpulse * (1.0 - std::max(a.exposure, b.exposure) * 0.48)
            : spring.tearImpulse;
        spring.stress = std::max(spring.stress * 0.9, std::max(0.0, stretchRatio - 1.0));

        if (stretchRatio > spring.tearStretch || (endpointLoad > tearImpulse && stretchRatio > 1.12)) {
            spring.broken = true;
            if (spring.layer == TissueLayer::Skin) {
                ++stats_.brokenSkin;
            } else {
                ++stats_.brokenMuscle;
            }
            a.load = std::max(a.load, endpointLoad * 0.35);
            b.load = std::max(b.load, endpointLoad * 0.35);
            continue;
        }

        const double diff = (len - spring.rest) / len;
        applyPairCorrection(a, b, dx * diff * spring.stiffness, dy * diff * spring.stiffness);
    }
}

void World::solveAttachments() {
    for (Attachment& attachment : attachments_) {
        if (attachment.broken) {
            continue;
        }

        Point& skin = points_[attachment.skinPoint];
        Point& muscle = points_[attachment.musclePoint];
        const double dx = muscle.position.x - skin.position.x;
        const double dy = muscle.position.y - skin.position.y;
        const double len = std::sqrt(dx * dx + dy * dy);
        if (len < kEpsilon) {
            continue;
        }

        const double stretchRatio = len / std::max(1.0, attachment.rest);
        const double impulse = std::max(skin.load, muscle.load);
        attachment.stress = std::max(attachment.stress * 0.88, std::max(0.0, stretchRatio - 1.0));

        if (stretchRatio > materials_.attachmentBreakStretch || (impulse > materials_.attachmentBreakImpulse && stretchRatio > 1.25)) {
            attachment.broken = true;
            ++stats_.brokenAttachments;
            skin.exposure = std::max(skin.exposure, 1.0);
            muscle.exposure = std::max(muscle.exposure, 1.0);
            continue;
        }

        const double diff = (len - attachment.rest) / len;
        applyPairCorrection(skin, muscle, dx * diff * materials_.attachmentStiffness, dy * diff * materials_.attachmentStiffness);
    }
}

void World::solveAreas() {
    for (const AreaConstraint& area : areas_) {
        Triangle probe;
        probe.a = area.a;
        probe.b = area.b;
        probe.c = area.c;
        probe.layer = area.layer;
        if (!triangleAlive(probe)) {
            continue;
        }

        Point& a = points_[area.a];
        Point& b = points_[area.b];
        Point& c = points_[area.c];
        const double current = signedArea(a.position, b.position, c.position);
        const double constraint = current - area.restArea;
        const double invMassA = a.pinned ? 0.0 : 1.0 / a.mass;
        const double invMassB = b.pinned ? 0.0 : 1.0 / b.mass;
        const double invMassC = c.pinned ? 0.0 : 1.0 / c.mass;

        const double ax = b.position.y - c.position.y;
        const double ay = c.position.x - b.position.x;
        const double bx = c.position.y - a.position.y;
        const double by = a.position.x - c.position.x;
        const double cx = a.position.y - b.position.y;
        const double cy = b.position.x - a.position.x;
        const double weightedGradient =
            invMassA * (ax * ax + ay * ay) +
            invMassB * (bx * bx + by * by) +
            invMassC * (cx * cx + cy * cy);
        if (weightedGradient <= kEpsilon) {
            continue;
        }

        const double lambda = -constraint * area.stiffness / weightedGradient;
        if (!a.pinned) {
            a.position.x += ax * lambda * invMassA;
            a.position.y += ay * lambda * invMassA;
        }
        if (!b.pinned) {
            b.position.x += bx * lambda * invMassB;
            b.position.y += by * lambda * invMassB;
        }
        if (!c.pinned) {
            c.position.x += cx * lambda * invMassC;
            c.position.y += cy * lambda * invMassC;
        }
    }
}

void World::constrainToWorld(double width, double floorY) {
    constexpr double margin = 8.0;
    for (Point& point : points_) {
        if (point.pinned) {
            continue;
        }
        point.position.x = std::clamp(point.position.x, margin, width - margin);
        point.position.y = std::min(point.position.y, floorY);
    }
}

void World::updateExposure() {
    for (const Attachment& attachment : attachments_) {
        if (!attachment.broken) {
            continue;
        }
        points_[attachment.skinPoint].exposure = std::max(points_[attachment.skinPoint].exposure, 1.0);
        points_[attachment.musclePoint].exposure = std::max(points_[attachment.musclePoint].exposure, 1.0);
    }

    for (const Spring& spring : springs_) {
        if (!spring.broken || spring.layer != TissueLayer::Skin) {
            continue;
        }
        points_[spring.a].exposure = std::max(points_[spring.a].exposure, 0.85);
        points_[spring.b].exposure = std::max(points_[spring.b].exposure, 0.85);
    }
}

void World::updateTriangleDamage() {
    for (Triangle& triangle : triangles_) {
        if (triangle.layer != TissueLayer::Muscle || triangle.failed) {
            continue;
        }

        const Point& a = points_[triangle.a];
        const Point& b = points_[triangle.b];
        const Point& c = points_[triangle.c];
        const double load = (a.load + b.load + c.load) / 3.0;
        const double exposed = (a.exposure + b.exposure + c.exposure) / 3.0;
        const double impulseThreshold = materials_.muscleExposedTearImpulse + (1.0 - exposed) * 560.0;
        triangle.damage = std::min(1.35, triangle.damage * 0.996 + std::max(0.0, load - impulseThreshold) / 1500.0);
        if (triangle.damage > 1.0) {
            triangle.failed = true;
        }
    }
}

World createLayeredBody(double width, double height, Materials materials) {
    World world(materials);
    const double bodyHeight = std::min({height * 0.78, width * 1.12, 720.0});
    const double bodyWidth = bodyHeight * 0.64;
    const double originX = width * 0.52;
    const double originY = height * 0.09;
    const int cols = std::max(3, static_cast<int>(std::floor(bodyWidth / materials.pointSpacing)));
    const int rows = std::max(3, static_cast<int>(std::floor(bodyHeight / materials.pointSpacing)));

    std::unordered_map<GridKey, std::size_t, GridKeyHash> skinGrid;
    std::unordered_map<GridKey, std::size_t, GridKeyHash> muscleGrid;

    for (int y = 0; y <= rows; ++y) {
        for (int x = 0; x <= cols; ++x) {
            const double nx = (static_cast<double>(x) / cols - 0.5) * 0.7;
            const double ny = static_cast<double>(y) / rows;
            if (!isInsideHumanoid(nx, ny)) {
                continue;
            }

            const double skinX = originX + (static_cast<double>(x) / cols - 0.5) * bodyWidth;
            const double skinY = originY + ny * bodyHeight;
            const double muscleX = originX + (static_cast<double>(x) / cols - 0.5) * bodyWidth * 0.78;
            const double muscleY = originY + ny * bodyHeight;
            const bool pinned = ny < 0.035;
            const GridKey key{x, y};

            const std::size_t skinPoint = world.addPoint({skinX, skinY}, TissueLayer::Skin, pinned);
            const std::size_t musclePoint = world.addPoint({muscleX, muscleY}, TissueLayer::Muscle, pinned);
            skinGrid[key] = skinPoint;
            muscleGrid[key] = musclePoint;
            world.addAttachment(skinPoint, musclePoint);
        }
    }

    auto get = [](const std::unordered_map<GridKey, std::size_t, GridKeyHash>& grid, int x, int y) -> std::size_t {
        const auto it = grid.find({x, y});
        return it == grid.end() ? std::numeric_limits<std::size_t>::max() : it->second;
    };

    auto addSkinSpring = [&](std::size_t a, std::size_t b, double stiffness, double tearStretch) {
        if (a == std::numeric_limits<std::size_t>::max() || b == std::numeric_limits<std::size_t>::max()) {
            return;
        }
        world.addSpring(a, b, TissueLayer::Skin, stiffness, tearStretch, materials.skinTearImpulse);
    };

    auto addMuscleSpring = [&](std::size_t a, std::size_t b, double stiffness, bool fiber, double tearStretch) {
        if (a == std::numeric_limits<std::size_t>::max() || b == std::numeric_limits<std::size_t>::max()) {
            return;
        }
        world.addSpring(a, b, TissueLayer::Muscle, stiffness, tearStretch, materials.muscleTearImpulse, fiber);
    };

    for (int y = 0; y <= rows; ++y) {
        for (int x = 0; x <= cols; ++x) {
            const std::size_t skinPoint = get(skinGrid, x, y);
            const std::size_t musclePoint = get(muscleGrid, x, y);
            if (skinPoint == std::numeric_limits<std::size_t>::max() || musclePoint == std::numeric_limits<std::size_t>::max()) {
                continue;
            }

            addSkinSpring(skinPoint, get(skinGrid, x + 1, y), materials.skinStructuralStiffness, materials.skinTearStretch);
            addSkinSpring(skinPoint, get(skinGrid, x, y + 1), materials.skinStructuralStiffness, materials.skinTearStretch);
            addSkinSpring(skinPoint, get(skinGrid, x + 1, y + 1), materials.skinShearStiffness, materials.skinTearStretch * 1.08);
            addSkinSpring(skinPoint, get(skinGrid, x - 1, y + 1), materials.skinShearStiffness, materials.skinTearStretch * 1.08);

            addMuscleSpring(musclePoint, get(muscleGrid, x, y + 1), materials.muscleFiberStiffness, true, materials.muscleTearStretch);
            addMuscleSpring(musclePoint, get(muscleGrid, x, y + 2), materials.muscleFiberStiffness * 0.42, true, materials.muscleTearStretch * 1.05);
            addMuscleSpring(musclePoint, get(muscleGrid, x + 1, y), materials.muscleCrossStiffness, false, materials.muscleTearStretch);
            addMuscleSpring(musclePoint, get(muscleGrid, x + 1, y + 1), materials.muscleShearStiffness, false, materials.muscleTearStretch * 1.12);
            addMuscleSpring(musclePoint, get(muscleGrid, x - 1, y + 1), materials.muscleShearStiffness, false, materials.muscleTearStretch * 1.12);
        }
    }

    for (int y = 0; y < rows; ++y) {
        for (int x = 0; x < cols; ++x) {
            addCellTriangles(world, skinGrid, x, y, TissueLayer::Skin, materials.skinAreaStiffness);
            addCellTriangles(world, muscleGrid, x, y, TissueLayer::Muscle, materials.muscleAreaStiffness);
        }
    }

    return world;
}

} // namespace rp
