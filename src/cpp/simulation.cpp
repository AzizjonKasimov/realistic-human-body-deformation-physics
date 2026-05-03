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

bool pointInTriangle(Vec2 p, Vec2 a, Vec2 b, Vec2 c) {
    const double d1 = (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
    const double d2 = (p.x - c.x) * (b.y - c.y) - (b.x - c.x) * (p.y - c.y);
    const double d3 = (p.x - a.x) * (c.y - a.y) - (c.x - a.x) * (p.y - a.y);
    const bool hasNegative = d1 < -kEpsilon || d2 < -kEpsilon || d3 < -kEpsilon;
    const bool hasPositive = d1 > kEpsilon || d2 > kEpsilon || d3 > kEpsilon;
    return !(hasNegative && hasPositive);
}

double distanceToSegment(Vec2 point, Vec2 a, Vec2 b) {
    const double abx = b.x - a.x;
    const double aby = b.y - a.y;
    const double apx = point.x - a.x;
    const double apy = point.y - a.y;
    const double abLenSq = abx * abx + aby * aby;
    if (abLenSq <= kEpsilon) {
        return distance(point, a);
    }
    const double t = std::clamp((apx * abx + apy * aby) / abLenSq, 0.0, 1.0);
    const Vec2 closest{a.x + abx * t, a.y + aby * t};
    return distance(point, closest);
}

double segmentT(Vec2 point, Vec2 a, Vec2 b) {
    const double abx = b.x - a.x;
    const double aby = b.y - a.y;
    const double abLenSq = abx * abx + aby * aby;
    if (abLenSq <= kEpsilon) {
        return 0.0;
    }
    return std::clamp(((point.x - a.x) * abx + (point.y - a.y) * aby) / abLenSq, 0.0, 1.0);
}

bool isInsideHumanoidLayer(double nx, double ny, double inset) {
    const double s = std::clamp(1.0 - inset, 0.2, 1.0);
    const bool head = ellipse(nx, ny, 0.0, 0.105, 0.078 * s, 0.085 * s);
    const bool neck = box(nx, ny, -0.034 * s, 0.034 * s, 0.17, 0.25);
    const bool shoulders = ellipse(nx, ny, 0.0, 0.275, 0.205 * s, 0.075 * s);
    const bool chest = ellipse(nx, ny, 0.0, 0.43, 0.155 * s, 0.225 * s);
    const bool hips = ellipse(nx, ny, 0.0, 0.64, 0.132 * s, 0.11 * s);
    const bool leftArm = capsule(nx, ny, -0.195, 0.285, -0.245, 0.62, 0.052 * s);
    const bool rightArm = capsule(nx, ny, 0.195, 0.285, 0.245, 0.62, 0.052 * s);
    const bool leftLeg = capsule(nx, ny, -0.065, 0.675, -0.082, 0.97, 0.056 * s);
    const bool rightLeg = capsule(nx, ny, 0.065, 0.675, 0.082, 0.97, 0.056 * s);
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

bool pointInsideLayer(const World& world, Vec2 point, TissueLayer layer) {
    const std::vector<Point>& points = world.points();
    for (const Triangle& triangle : world.triangles()) {
        if (triangle.layer != layer || !world.triangleAlive(triangle)) {
            continue;
        }
        if (pointInTriangle(point, points[triangle.a].position, points[triangle.b].position, points[triangle.c].position)) {
            return true;
        }
    }
    return false;
}

AnatomyValidation validateAnatomy(const World& world, int samplesPerBone) {
    AnatomyValidation validation;
    validation.skinPoints = static_cast<int>(std::count_if(world.points().begin(), world.points().end(), [](const Point& point) {
        return point.layer == TissueLayer::Skin;
    }));
    validation.musclePoints = static_cast<int>(std::count_if(world.points().begin(), world.points().end(), [](const Point& point) {
        return point.layer == TissueLayer::Muscle;
    }));

    const int sampleCount = std::max(2, samplesPerBone);
    for (const BoneSegment& bone : world.bones()) {
        bool segmentOutsideSkin = false;
        bool segmentOutsideMuscle = false;
        for (int i = 0; i < sampleCount; ++i) {
            const double t = sampleCount == 1 ? 0.0 : static_cast<double>(i) / static_cast<double>(sampleCount - 1);
            const Vec2 sample{bone.a.x + (bone.b.x - bone.a.x) * t, bone.a.y + (bone.b.y - bone.a.y) * t};
            ++validation.boneSamples;
            if (!pointInsideLayer(world, sample, TissueLayer::Skin)) {
                ++validation.boneSamplesOutsideSkin;
                segmentOutsideSkin = true;
            }
            if (!pointInsideLayer(world, sample, TissueLayer::Muscle)) {
                ++validation.boneSamplesOutsideMuscle;
                segmentOutsideMuscle = true;
            }
        }
        if (segmentOutsideSkin) {
            ++validation.boneSegmentsOutsideSkin;
        }
        if (segmentOutsideMuscle) {
            ++validation.boneSegmentsOutsideMuscle;
        }
    }

    return validation;
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
    area.edgeAB = findSpringIndex(a, b, layer);
    area.edgeBC = findSpringIndex(b, c, layer);
    area.edgeCA = findSpringIndex(c, a, layer);
    area.layer = layer;
    area.restArea = signedArea(points_[a].position, points_[b].position, points_[c].position);
    area.stiffness = stiffness;
    areas_.push_back(area);
}

void World::addAttachment(std::size_t skinPoint, std::size_t musclePoint) {
    Attachment attachment;
    attachment.skinPoint = skinPoint;
    attachment.musclePoint = musclePoint;
    attachment.rest = std::max(materials_.pointSpacing * 0.45, distance(points_[skinPoint].position, points_[musclePoint].position));
    attachments_.push_back(attachment);
}

void World::addTriangle(std::size_t a, std::size_t b, std::size_t c, TissueLayer layer) {
    Triangle triangle;
    triangle.a = a;
    triangle.b = b;
    triangle.c = c;
    triangle.edgeAB = findSpringIndex(a, b, layer);
    triangle.edgeBC = findSpringIndex(b, c, layer);
    triangle.edgeCA = findSpringIndex(c, a, layer);
    triangle.layer = layer;
    triangles_.push_back(triangle);
}

std::size_t World::addBoneSegment(Vec2 a, Vec2 b, double radius, double fractureImpulse, bool pinned) {
    BoneSegment bone;
    bone.a = a;
    bone.b = b;
    bone.previousA = a;
    bone.previousB = b;
    bone.homeA = a;
    bone.homeB = b;
    bone.radius = radius;
    bone.restLength = std::max(kEpsilon, distance(a, b));
    bone.fractureImpulse = fractureImpulse;
    bone.pinned = pinned;
    const std::size_t index = bones_.size();
    bones_.push_back(bone);
    return index;
}

void World::addBoneAttachment(std::size_t point, std::size_t bone, double t) {
    if (point >= points_.size() || bone >= bones_.size()) {
        return;
    }

    BoneAttachment attachment;
    attachment.point = point;
    attachment.bone = bone;
    attachment.t = std::clamp(t, 0.0, 1.0);
    const Vec2 anchor = bonePoint(bones_[bone], attachment.t);
    attachment.offset = {points_[point].position.x - anchor.x, points_[point].position.y - anchor.y};
    attachment.rest = std::max(1.0, distance(points_[point].position, anchor));
    boneAttachments_.push_back(attachment);
}

void World::step(double dt, const InputState& input, double width, double height) {
    const double floorY = height - 38.0;
    updateExposure();
    integrate(dt, width, floorY);
    collideStriker(dt, input);

    for (int i = 0; i < materials_.solverIterations; ++i) {
        solveSprings();
        solveAttachments();
        solveBoneAttachments();
        solveBones();
        solveAreas();
        constrainToWorld(width, floorY);
    }

    updateTriangleDamage();
}

bool World::triangleAlive(const Triangle& triangle) const {
    if (triangle.failed) {
        return false;
    }

    return liveEdgeCount(triangle.edgeAB, triangle.edgeBC, triangle.edgeCA) >= 2;
}

bool World::hasLiveSpring(std::size_t a, std::size_t b, TissueLayer layer) const {
    return springAlive(findSpringIndex(a, b, layer));
}

std::size_t World::findSpringIndex(std::size_t a, std::size_t b, TissueLayer layer) const {
    for (std::size_t i = 0; i < springs_.size(); ++i) {
        const Spring& spring = springs_[i];
        if (spring.layer != layer) {
            continue;
        }
        if ((spring.a == a && spring.b == b) || (spring.a == b && spring.b == a)) {
            return i;
        }
    }
    return kMissingSpring;
}

bool World::springAlive(std::size_t springIndex) const {
    return springIndex != kMissingSpring && springIndex < springs_.size() && !springs_[springIndex].broken;
}

int World::liveEdgeCount(std::size_t edgeAB, std::size_t edgeBC, std::size_t edgeCA) const {
    return (springAlive(edgeAB) ? 1 : 0) + (springAlive(edgeBC) ? 1 : 0) + (springAlive(edgeCA) ? 1 : 0);
}

Vec2 World::bonePoint(const BoneSegment& bone, double t) const {
    return {
        bone.a.x + (bone.b.x - bone.a.x) * t,
        bone.a.y + (bone.b.y - bone.a.y) * t,
    };
}

void World::fractureBone(std::size_t boneIndex) {
    if (boneIndex >= bones_.size() || bones_[boneIndex].fractured) {
        return;
    }

    BoneSegment& bone = bones_[boneIndex];
    const Vec2 oldA = bone.a;
    const Vec2 oldB = bone.b;
    const Vec2 oldPreviousA = bone.previousA;
    const Vec2 oldPreviousB = bone.previousB;
    const Vec2 oldHomeA = bone.homeA;
    const Vec2 oldHomeB = bone.homeB;
    const double dx = oldB.x - oldA.x;
    const double dy = oldB.y - oldA.y;
    const double len = std::max(kEpsilon, std::sqrt(dx * dx + dy * dy));
    const Vec2 dir{dx / len, dy / len};
    const Vec2 mid{(oldA.x + oldB.x) * 0.5, (oldA.y + oldB.y) * 0.5};
    const double gap = std::min(8.0, len * 0.08);

    bone.b = {mid.x - dir.x * gap, mid.y - dir.y * gap};
    bone.previousB = {(oldPreviousA.x + oldPreviousB.x) * 0.5 - dir.x * gap, (oldPreviousA.y + oldPreviousB.y) * 0.5 - dir.y * gap};
    bone.homeB = {(oldHomeA.x + oldHomeB.x) * 0.5 - dir.x * gap, (oldHomeA.y + oldHomeB.y) * 0.5 - dir.y * gap};
    bone.restLength = std::max(kEpsilon, distance(bone.a, bone.b));
    bone.fractured = true;

    BoneSegment second;
    second.a = {mid.x + dir.x * gap, mid.y + dir.y * gap};
    second.b = oldB;
    second.previousA = {(oldPreviousA.x + oldPreviousB.x) * 0.5 + dir.x * gap, (oldPreviousA.y + oldPreviousB.y) * 0.5 + dir.y * gap};
    second.previousB = oldPreviousB;
    second.homeA = {(oldHomeA.x + oldHomeB.x) * 0.5 + dir.x * gap, (oldHomeA.y + oldHomeB.y) * 0.5 + dir.y * gap};
    second.homeB = oldHomeB;
    second.radius = bone.radius;
    second.restLength = std::max(kEpsilon, distance(second.a, second.b));
    second.fractureImpulse = bone.fractureImpulse;
    second.load = bone.load;
    second.fractured = true;
    second.pinned = bone.pinned;
    const std::size_t secondIndex = bones_.size();
    bones_.push_back(second);

    for (BoneAttachment& attachment : boneAttachments_) {
        if (attachment.bone != boneIndex || attachment.broken) {
            continue;
        }
        if (attachment.t <= 0.5) {
            attachment.t = std::clamp(attachment.t * 2.0, 0.0, 1.0);
            const Vec2 anchor = bonePoint(bones_[boneIndex], attachment.t);
            attachment.offset = {points_[attachment.point].position.x - anchor.x, points_[attachment.point].position.y - anchor.y};
            attachment.rest = std::max(1.0, distance(points_[attachment.point].position, anchor));
        } else {
            attachment.bone = secondIndex;
            attachment.t = std::clamp((attachment.t - 0.5) * 2.0, 0.0, 1.0);
            const Vec2 anchor = bonePoint(bones_[secondIndex], attachment.t);
            attachment.offset = {points_[attachment.point].position.x - anchor.x, points_[attachment.point].position.y - anchor.y};
            attachment.rest = std::max(1.0, distance(points_[attachment.point].position, anchor));
        }
    }

    ++stats_.fracturedBones;
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

    for (BoneSegment& bone : bones_) {
        bone.load *= 0.88;
        if (bone.pinned) {
            bone.a = bone.homeA;
            bone.b = bone.homeB;
            bone.previousA = bone.a;
            bone.previousB = bone.b;
            continue;
        }

        const double avx = (bone.a.x - bone.previousA.x) * materials_.boneDamping;
        const double avy = (bone.a.y - bone.previousA.y) * materials_.boneDamping;
        const double bvx = (bone.b.x - bone.previousB.x) * materials_.boneDamping;
        const double bvy = (bone.b.y - bone.previousB.y) * materials_.boneDamping;
        bone.previousA = bone.a;
        bone.previousB = bone.b;
        bone.a.x += avx + (bone.homeA.x - bone.a.x) * materials_.boneShapeStiffness;
        bone.a.y += avy + materials_.gravity * dt * dt + (bone.homeA.y - bone.a.y) * materials_.boneShapeStiffness;
        bone.b.x += bvx + (bone.homeB.x - bone.b.x) * materials_.boneShapeStiffness;
        bone.b.y += bvy + materials_.gravity * dt * dt + (bone.homeB.y - bone.b.y) * materials_.boneShapeStiffness;
    }
}

void World::collideStriker(double dt, const InputState& input) {
    if (!input.active) {
        return;
    }

    const double speed = std::sqrt(input.vx * input.vx + input.vy * input.vy);
    const double impact = speed * materials_.strikerMass * input.power;
    const double influence = materials_.strikerRadius + 12.0;

    for (std::size_t i = 0; i < bones_.size(); ++i) {
        BoneSegment& bone = bones_[i];
        bone.load *= 0.88;
        const double dist = distanceToSegment({input.x, input.y}, bone.a, bone.b);
        if (!input.down || dist > influence + bone.radius) {
            continue;
        }
        const double contact = 1.0 - std::clamp((dist - bone.radius) / influence, 0.0, 1.0);
        bone.load = std::max(bone.load, impact * contact);
        if (!bone.fractured && bone.load > bone.fractureImpulse) {
            fractureBone(i);
        }
    }

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

void World::solveBoneAttachments() {
    for (BoneAttachment& attachment : boneAttachments_) {
        if (attachment.broken || attachment.point >= points_.size() || attachment.bone >= bones_.size()) {
            continue;
        }

        Point& point = points_[attachment.point];
        BoneSegment& bone = bones_[attachment.bone];
        const Vec2 rawAnchor = bonePoint(bone, attachment.t);
        const double currentBoneDistance = distance(point.position, rawAnchor);
        const double stretchRatio = currentBoneDistance / std::max(1.0, attachment.rest);
        const double impulse = std::max(point.load, bone.load);
        attachment.stress = std::max(attachment.stress * 0.9, std::max(0.0, stretchRatio - 1.0));

        if (stretchRatio > materials_.boneAttachmentBreakStretch || (impulse > materials_.boneAttachmentBreakImpulse && stretchRatio > 1.45)) {
            attachment.broken = true;
            ++stats_.brokenBoneAttachments;
            point.exposure = std::max(point.exposure, 0.85);
            continue;
        }

        const Vec2 target{rawAnchor.x + attachment.offset.x, rawAnchor.y + attachment.offset.y};
        const double correctionX = (target.x - point.position.x) * materials_.boneAttachmentStiffness;
        const double correctionY = (target.y - point.position.y) * materials_.boneAttachmentStiffness;
        if (!point.pinned) {
            point.position.x += correctionX;
            point.position.y += correctionY;
        }

        if (!bone.pinned) {
            const double boneShare = 0.10;
            const double aWeight = 1.0 - attachment.t;
            const double bWeight = attachment.t;
            bone.a.x -= correctionX * boneShare * aWeight;
            bone.a.y -= correctionY * boneShare * aWeight;
            bone.b.x -= correctionX * boneShare * bWeight;
            bone.b.y -= correctionY * boneShare * bWeight;
        }
    }
}

void World::solveBones() {
    for (BoneSegment& bone : bones_) {
        if (bone.pinned) {
            bone.a = bone.homeA;
            bone.b = bone.homeB;
            bone.previousA = bone.a;
            bone.previousB = bone.b;
            continue;
        }

        const double dx = bone.b.x - bone.a.x;
        const double dy = bone.b.y - bone.a.y;
        const double len = std::sqrt(dx * dx + dy * dy);
        if (len < kEpsilon) {
            continue;
        }

        const double diff = (len - bone.restLength) / len;
        const double correctionX = dx * diff * 0.5;
        const double correctionY = dy * diff * 0.5;
        bone.a.x += correctionX;
        bone.a.y += correctionY;
        bone.b.x -= correctionX;
        bone.b.y -= correctionY;
    }
}

void World::solveAreas() {
    for (const AreaConstraint& area : areas_) {
        if (liveEdgeCount(area.edgeAB, area.edgeBC, area.edgeCA) < 2) {
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
    for (BoneSegment& bone : bones_) {
        if (bone.pinned) {
            continue;
        }
        bone.a.x = std::clamp(bone.a.x, margin, width - margin);
        bone.b.x = std::clamp(bone.b.x, margin, width - margin);
        bone.a.y = std::min(bone.a.y, floorY);
        bone.b.y = std::min(bone.b.y, floorY);
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
    std::vector<std::size_t> skinPoints;
    std::vector<std::size_t> musclePoints;

    for (int y = 0; y <= rows; ++y) {
        for (int x = 0; x <= cols; ++x) {
            const double nx = (static_cast<double>(x) / cols - 0.5) * 0.7;
            const double ny = static_cast<double>(y) / rows;
            const double worldX = originX + (static_cast<double>(x) / cols - 0.5) * bodyWidth;
            const double worldY = originY + ny * bodyHeight;
            const bool pinned = ny < 0.035;
            const GridKey key{x, y};

            if (isInsideHumanoidLayer(nx, ny, 0.0)) {
                const std::size_t skinPoint = world.addPoint({worldX, worldY}, TissueLayer::Skin, pinned);
                skinGrid[key] = skinPoint;
                skinPoints.push_back(skinPoint);
            }

            if (isInsideHumanoidLayer(nx, ny, 0.24)) {
                const std::size_t musclePoint = world.addPoint({worldX, worldY}, TissueLayer::Muscle, pinned);
                muscleGrid[key] = musclePoint;
                musclePoints.push_back(musclePoint);
            }
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
            if (skinPoint == std::numeric_limits<std::size_t>::max()) {
                continue;
            }

            addSkinSpring(skinPoint, get(skinGrid, x + 1, y), materials.skinStructuralStiffness, materials.skinTearStretch);
            addSkinSpring(skinPoint, get(skinGrid, x, y + 1), materials.skinStructuralStiffness, materials.skinTearStretch);
            addSkinSpring(skinPoint, get(skinGrid, x + 1, y + 1), materials.skinShearStiffness, materials.skinTearStretch * 1.08);
            addSkinSpring(skinPoint, get(skinGrid, x - 1, y + 1), materials.skinShearStiffness, materials.skinTearStretch * 1.08);

            if (musclePoint == std::numeric_limits<std::size_t>::max()) {
                continue;
            }

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

    for (std::size_t skinPoint : skinPoints) {
        std::size_t nearest = std::numeric_limits<std::size_t>::max();
        double nearestDistance = materials.pointSpacing * 2.35;
        for (std::size_t musclePoint : musclePoints) {
            const double d = distance(world.points()[skinPoint].position, world.points()[musclePoint].position);
            if (d < nearestDistance) {
                nearestDistance = d;
                nearest = musclePoint;
            }
        }
        if (nearest != std::numeric_limits<std::size_t>::max()) {
            world.addAttachment(skinPoint, nearest);
        }
    }

    const auto bodyPoint = [&](double nx, double ny) {
        return Vec2{originX + (nx / 0.7) * bodyWidth, originY + ny * bodyHeight};
    };

    world.addBoneSegment(bodyPoint(0.0, 0.06), bodyPoint(0.0, 0.17), 10.0, materials.boneFractureImpulse * 0.75, true);
    world.addBoneSegment(bodyPoint(0.0, 0.20), bodyPoint(0.0, 0.63), 8.0, materials.boneFractureImpulse);
    world.addBoneSegment(bodyPoint(-0.14, 0.30), bodyPoint(0.14, 0.30), 7.0, materials.boneFractureImpulse * 0.95);
    world.addBoneSegment(bodyPoint(-0.10, 0.48), bodyPoint(0.10, 0.48), 6.0, materials.boneFractureImpulse * 0.9);
    world.addBoneSegment(bodyPoint(-0.195, 0.295), bodyPoint(-0.245, 0.60), 7.0, materials.boneFractureImpulse * 0.82);
    world.addBoneSegment(bodyPoint(0.195, 0.295), bodyPoint(0.245, 0.60), 7.0, materials.boneFractureImpulse * 0.82);
    world.addBoneSegment(bodyPoint(-0.065, 0.68), bodyPoint(-0.082, 0.95), 8.0, materials.boneFractureImpulse * 0.9);
    world.addBoneSegment(bodyPoint(0.065, 0.68), bodyPoint(0.082, 0.95), 8.0, materials.boneFractureImpulse * 0.9);

    for (std::size_t musclePoint : musclePoints) {
        std::size_t nearestBone = std::numeric_limits<std::size_t>::max();
        double nearestDistance = materials.pointSpacing * 2.7;
        double nearestT = 0.0;
        for (std::size_t i = 0; i < world.bones().size(); ++i) {
            const rp::BoneSegment& bone = world.bones()[i];
            const double t = segmentT(world.points()[musclePoint].position, bone.a, bone.b);
            const Vec2 anchor = {bone.a.x + (bone.b.x - bone.a.x) * t, bone.a.y + (bone.b.y - bone.a.y) * t};
            const double d = distance(world.points()[musclePoint].position, anchor);
            if (d < nearestDistance) {
                nearestDistance = d;
                nearestBone = i;
                nearestT = t;
            }
        }
        if (nearestBone != std::numeric_limits<std::size_t>::max()) {
            world.addBoneAttachment(musclePoint, nearestBone, nearestT);
        }
    }

    return world;
}

} // namespace rp
