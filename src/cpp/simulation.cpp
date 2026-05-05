#include "simulation.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <string>
#include <unordered_map>

namespace rp {
namespace {

constexpr double kEpsilon = 0.0001;
constexpr double kPi = 3.14159265358979323846;

struct ToolProfile {
    double radiusScale = 1.0;
    double reachPadding = 12.0;
    double massScale = 1.0;
    double tissuePushScale = 1.0;
    double tissueLoadScale = 1.0;
    double bonePushScale = 1.0;
    double boneLoadScale = 1.0;
    double fractureScale = 1.0;
    double tearPressureScale = 0.0;
    double fluidScale = 1.0;
    double bladeNormalBias = 0.0;
    double dragScale = 1.0;
    double reboundScale = 1.0;
    double bladeFrontScale = 0.0;
    double bladeBackScale = 0.0;
    double bladeContactRadiusScale = 1.0;
};

struct ToolContactShape {
    Vec2 center;
    Vec2 axisStart;
    Vec2 axisEnd;
    Vec2 direction;
    Vec2 bladeNormal;
    double influence = 0.0;
    bool bladeSegment = false;
};

struct ToolPointContact {
    Vec2 contactPoint;
    Vec2 normal;
    double distance = 0.0;
};

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

struct SegmentClosestPoints {
    double tA = 0.0;
    double tB = 0.0;
    Vec2 pointA;
    Vec2 pointB;
    double distance = 0.0;
};

ToolProfile toolProfile(ToolMode tool) {
    switch (tool) {
    case ToolMode::Sharp:
        return {
            0.48,
            5.0,
            0.72,
            0.38,
            2.05,
            0.30,
            0.58,
            0.76,
            1.0,
            1.35,
            0.82,
            0.72,
            0.58,
            1.55,
            0.65,
            0.42,
        };
    case ToolMode::Heavy:
        return {
            1.18,
            16.0,
            1.85,
            1.24,
            1.18,
            1.46,
            1.72,
            1.34,
            0.10,
            0.92,
            0.0,
            0.82,
            1.42,
            0.0,
            0.0,
            1.0,
        };
    case ToolMode::Blunt:
    default:
        return {};
    }
}

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

double segmentT(Vec2 point, Vec2 a, Vec2 b) {
    const double abx = b.x - a.x;
    const double aby = b.y - a.y;
    const double abLenSq = abx * abx + aby * aby;
    if (abLenSq <= kEpsilon) {
        return 0.0;
    }
    return std::clamp(((point.x - a.x) * abx + (point.y - a.y) * aby) / abLenSq, 0.0, 1.0);
}

Vec2 lerp(Vec2 a, Vec2 b, double t) {
    return {
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
    };
}

double wrapAngle(double angle) {
    while (angle > kPi) {
        angle -= kPi * 2.0;
    }
    while (angle < -kPi) {
        angle += kPi * 2.0;
    }
    return angle;
}

double boneAngle(const BoneSegment& bone) {
    return std::atan2(bone.b.y - bone.a.y, bone.b.x - bone.a.x);
}

Vec2 rotateAround(Vec2 point, Vec2 pivot, double angle) {
    const double s = std::sin(angle);
    const double c = std::cos(angle);
    const double dx = point.x - pivot.x;
    const double dy = point.y - pivot.y;
    return {
        pivot.x + dx * c - dy * s,
        pivot.y + dx * s + dy * c,
    };
}

Vec2 add(Vec2 a, Vec2 b) {
    return {a.x + b.x, a.y + b.y};
}

Vec2 subtract(Vec2 a, Vec2 b) {
    return {a.x - b.x, a.y - b.y};
}

Vec2 scale(Vec2 value, double amount) {
    return {value.x * amount, value.y * amount};
}

double cross(Vec2 a, Vec2 b) {
    return a.x * b.y - a.y * b.x;
}

double dot(Vec2 a, Vec2 b) {
    return a.x * b.x + a.y * b.y;
}

Vec2 midpoint(Vec2 a, Vec2 b) {
    return {(a.x + b.x) * 0.5, (a.y + b.y) * 0.5};
}

Vec2 normalized(Vec2 value, Vec2 fallback) {
    const double len = std::sqrt(value.x * value.x + value.y * value.y);
    if (len <= kEpsilon) {
        return fallback;
    }
    return {value.x / len, value.y / len};
}

double distanceToSegment(Vec2 point, Vec2 a, Vec2 b) {
    const double t = segmentT(point, a, b);
    const Vec2 closest = lerp(a, b, t);
    return distance(point, closest);
}

SegmentClosestPoints closestSegmentPoints(Vec2 a0, Vec2 a1, Vec2 b0, Vec2 b1) {
    const Vec2 dA = subtract(a1, a0);
    const Vec2 dB = subtract(b1, b0);
    const Vec2 r = subtract(a0, b0);
    const double lenA = dot(dA, dA);
    const double lenB = dot(dB, dB);
    const double dBF = dot(dB, r);
    double tA = 0.0;
    double tB = 0.0;

    if (lenA <= kEpsilon && lenB <= kEpsilon) {
        tA = 0.0;
        tB = 0.0;
    } else if (lenA <= kEpsilon) {
        tA = 0.0;
        tB = std::clamp(dBF / lenB, 0.0, 1.0);
    } else {
        const double dAC = dot(dA, r);
        if (lenB <= kEpsilon) {
            tB = 0.0;
            tA = std::clamp(-dAC / lenA, 0.0, 1.0);
        } else {
            const double dAB = dot(dA, dB);
            const double denom = lenA * lenB - dAB * dAB;
            tA = denom > kEpsilon ? std::clamp((dAB * dBF - dAC * lenB) / denom, 0.0, 1.0) : 0.0;

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

    const Vec2 pointA = lerp(a0, a1, tA);
    const Vec2 pointB = lerp(b0, b1, tB);
    return {tA, tB, pointA, pointB, distance(pointA, pointB)};
}

ToolContactShape makeToolContactShape(const InputState& input, const ToolProfile& profile, double radius) {
    const Vec2 center{input.x, input.y};
    const Vec2 direction = normalized({input.vx, input.vy}, {1.0, 0.0});
    const Vec2 bladeNormal{-direction.y, direction.x};
    const bool bladeSegment = profile.bladeFrontScale > 0.0;
    const double contactRadius = bladeSegment ? radius * profile.bladeContactRadiusScale : radius;

    ToolContactShape shape;
    shape.center = center;
    shape.direction = direction;
    shape.bladeNormal = bladeNormal;
    shape.influence = contactRadius + profile.reachPadding;
    shape.bladeSegment = bladeSegment;
    shape.axisStart = bladeSegment ? subtract(center, scale(direction, radius * profile.bladeBackScale)) : center;
    shape.axisEnd = bladeSegment ? add(center, scale(direction, radius * profile.bladeFrontScale)) : center;
    return shape;
}

ToolPointContact samplePointContact(Vec2 point, const ToolContactShape& shape) {
    Vec2 contactPoint = shape.center;
    if (shape.bladeSegment) {
        contactPoint = lerp(shape.axisStart, shape.axisEnd, segmentT(point, shape.axisStart, shape.axisEnd));
    }

    const Vec2 delta = subtract(point, contactPoint);
    return {contactPoint, normalized(delta, shape.bladeNormal), distance(point, contactPoint)};
}

bool freeBoneFragment(const BoneSegment& bone) {
    return !bone.pinned && (bone.fractured || bone.splinter);
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

void World::addBoneJoint(std::size_t a, double tA, std::size_t b, double tB, double minAngle, double maxAngle) {
    if (a >= bones_.size() || b >= bones_.size() || a == b) {
        return;
    }

    BoneJoint joint;
    joint.a = a;
    joint.b = b;
    joint.tA = std::clamp(tA, 0.0, 1.0);
    joint.tB = std::clamp(tB, 0.0, 1.0);
    joint.rest = std::max(1.0, distance(bonePoint(bones_[a], joint.tA), bonePoint(bones_[b], joint.tB)));
    joint.restAngle = wrapAngle(boneAngle(bones_[b]) - boneAngle(bones_[a]));
    joint.minAngle = std::min(minAngle, maxAngle);
    joint.maxAngle = std::max(minAngle, maxAngle);
    boneJoints_.push_back(joint);
}

void World::step(double dt, const InputState& input, double width, double height) {
    const double floorY = height - 38.0;
    const ToolProfile profile = toolProfile(input.tool);
    const double strikerRadius = materials_.strikerRadius * profile.radiusScale;
    const double strikerMass = materials_.strikerMass * input.power * profile.massScale;
    const double strikerSpeed = std::sqrt(input.vx * input.vx + input.vy * input.vy);
    debug_ = {};
    debug_.active = input.active;
    debug_.down = input.down;
    debug_.strikerPosition = {input.x, input.y};
    debug_.strikerVelocity = {input.vx, input.vy};
    debug_.strikerSpeed = strikerSpeed;
    debug_.strikerMass = strikerMass;
    debug_.strikerRadius = strikerRadius;
    debug_.tool = input.tool;
    debug_.impact = strikerSpeed * strikerMass;
    updateExposure();
    integrate(dt, width, floorY);
    updateWounds(dt);
    collideStriker(dt, input);

    for (int i = 0; i < materials_.solverIterations; ++i) {
        solveSprings();
        solveAttachments();
        solveBoneAttachments();
        solveBoneJoints();
        solveBones();
        solvePostFractureJoints();
        solveBoneFragmentRepulsion();
        solveAreas();
        constrainToWorld(width, floorY);
    }

    collideBoneFragments();
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

bool World::canFractureBone(const BoneSegment& bone) const {
    if (bone.pinned || bone.splinter || bone.fractureGeneration >= materials_.maxBoneFractureDepth) {
        return false;
    }
    return bone.restLength >= materials_.minBoneFragmentLength * 2.0;
}

void World::applyBoneAnchorDelta(BoneSegment& bone, double t, double dx, double dy) {
    if (bone.pinned) {
        return;
    }

    const double aWeight = 1.0 - std::clamp(t, 0.0, 1.0);
    const double bWeight = std::clamp(t, 0.0, 1.0);
    bone.a.x += dx * aWeight;
    bone.a.y += dy * aWeight;
    bone.b.x += dx * bWeight;
    bone.b.y += dy * bWeight;
}

void World::rotateBoneAroundAnchor(BoneSegment& bone, double t, double angle) {
    if (bone.pinned || std::abs(angle) <= kEpsilon) {
        return;
    }

    const Vec2 anchor = bonePoint(bone, std::clamp(t, 0.0, 1.0));
    bone.a = rotateAround(bone.a, anchor, angle);
    bone.b = rotateAround(bone.b, anchor, angle);
}

void World::rotateBoneAroundCenter(BoneSegment& bone, double angle) {
    if (bone.pinned || std::abs(angle) <= kEpsilon) {
        return;
    }

    const Vec2 center = midpoint(bone.a, bone.b);
    bone.a = rotateAround(bone.a, center, angle);
    bone.b = rotateAround(bone.b, center, angle);
}

void World::applyBoneTorque(BoneSegment& bone, Vec2 contact, Vec2 impulse) {
    if (bone.pinned) {
        return;
    }

    const Vec2 center = midpoint(bone.a, bone.b);
    const Vec2 lever = subtract(contact, center);
    const double inertia = std::max(12.0, bone.restLength * bone.restLength * std::max(1.0, bone.radius));
    const double torque = cross(lever, impulse);
    bone.angularVelocity = std::clamp(bone.angularVelocity + torque / inertia * materials_.boneTorqueScale,
                                      -36.0,
                                      36.0);
    if (bone.fractured || bone.splinter) {
        debug_.maxBoneAngularSpeed = std::max(debug_.maxBoneAngularSpeed, std::abs(bone.angularVelocity));
    }
}

double World::nextFluidRandom() {
    fluidSeed_ = fluidSeed_ * 1664525U + 1013904223U;
    return static_cast<double>((fluidSeed_ >> 8U) & 0x00ffffffU) / static_cast<double>(0x01000000U);
}

void World::emitFluid(Vec2 center, Vec2 direction, int count, double speed, double radius, double intensity) {
    if (materials_.maxFluidParticles <= 0 || count <= 0) {
        return;
    }

    const int particleLimit = std::max(0, materials_.maxFluidParticles);
    const double dt = materials_.fixedDt;
    const Vec2 dir = normalized(direction, {0.0, -1.0});
    const Vec2 tangent{-dir.y, dir.x};
    const double clampedSpeed = std::clamp(speed, 55.0, 980.0);
    const double clampedRadius = std::clamp(radius, 1.35, 4.8);
    const double clampedIntensity = std::clamp(intensity, 0.35, 1.35);

    for (int i = 0; i < count; ++i) {
        const double spread = (nextFluidRandom() - 0.5) * 1.35;
        const double launch = clampedSpeed * (0.44 + nextFluidRandom() * 0.74);
        const double jitter = radius * (nextFluidRandom() - 0.5) * 2.4;
        const Vec2 velocity = add(scale(dir, launch), scale(tangent, spread * launch));
        FluidParticle particle;
        particle.position = add(center, scale(tangent, jitter));
        particle.previous = {
            particle.position.x - velocity.x * dt,
            particle.position.y - velocity.y * dt,
        };
        particle.radius = clampedRadius * (0.72 + nextFluidRandom() * 0.58);
        particle.maxLife = materials_.fluidLifetime * (0.62 + nextFluidRandom() * 0.76);
        particle.life = particle.maxLife;
        particle.intensity = clampedIntensity;
        particle.settled = false;

        if (static_cast<int>(fluids_.size()) < particleLimit) {
            fluids_.push_back(particle);
        } else {
            fluids_[fluidWriteCursor_ % fluids_.size()] = particle;
            fluidWriteCursor_ = (fluidWriteCursor_ + 1U) % fluids_.size();
        }
        ++stats_.emittedFluidParticles;
        ++debug_.fluidEmitted;
    }
}

void World::openWound(Vec2 center, Vec2 direction, TissueLayer layer, double pressure, double radius, double depth) {
    if (materials_.maxWoundSources <= 0 || pressure <= 0.0) {
        return;
    }

    const Vec2 dir = normalized(direction, {0.0, -1.0});
    const double clampedPressure = std::clamp(pressure, 0.12, 4.8);
    const double clampedDepth = std::clamp(depth, 0.12, 1.35);
    const double clampedRadius = std::clamp(radius, 1.3, 5.2);

    WoundSource* target = nullptr;
    double bestDistance = materials_.woundMergeRadius;
    for (WoundSource& wound : wounds_) {
        if (!wound.active) {
            if (target == nullptr) {
                target = &wound;
            }
            continue;
        }
        const double d = distance(wound.position, center);
        if (d < bestDistance) {
            bestDistance = d;
            target = &wound;
        }
    }

    if (target == nullptr) {
        if (static_cast<int>(wounds_.size()) < materials_.maxWoundSources) {
            wounds_.push_back({});
            target = &wounds_.back();
        } else {
            target = &*std::min_element(wounds_.begin(), wounds_.end(), [](const WoundSource& a, const WoundSource& b) {
                if (a.active != b.active) {
                    return !a.active;
                }
                return a.pressure * (1.0 - a.clot) < b.pressure * (1.0 - b.clot);
            });
        }
    }

    const bool wasActive = target->active;
    target->position = wasActive ? lerp(target->position, center, 0.35) : center;
    target->direction = normalized(add(scale(target->direction, wasActive ? 0.45 : 0.0), dir), dir);
    target->layer = target->layer == TissueLayer::Muscle || layer == TissueLayer::Muscle ? TissueLayer::Muscle : TissueLayer::Skin;
    target->pressure = std::min(6.0, std::max(target->pressure * 0.72, clampedPressure) + clampedPressure * 0.34);
    target->clot = wasActive ? std::max(0.0, target->clot - clampedPressure * 0.045) : 0.0;
    target->age = wasActive ? std::min(target->age, 0.45) : 0.0;
    target->radius = std::max(target->radius, clampedRadius);
    target->depth = std::max(target->depth, clampedDepth);
    target->active = true;
    if (!wasActive) {
        ++stats_.openedWounds;
    }
}

void World::updateWounds(double dt) {
    for (WoundSource& wound : wounds_) {
        if (!wound.active) {
            continue;
        }

        wound.age += dt;
        const double layerScale = wound.layer == TissueLayer::Muscle ? 1.35 : 0.78;
        const double openFactor = std::max(0.0, 1.0 - wound.clot);
        debug_.maxWoundPressure = std::max(debug_.maxWoundPressure, wound.pressure);
        debug_.maxWoundClot = std::max(debug_.maxWoundClot, wound.clot);

        wound.accumulator += dt * materials_.woundLeakRate * wound.pressure * openFactor *
                             layerScale * (0.45 + wound.depth * 0.82);
        if (wound.age < 0.42 && wound.pressure > materials_.woundSprayPressure) {
            wound.accumulator += dt * (wound.pressure - materials_.woundSprayPressure) * 2.1;
        }

        int count = std::min(4, static_cast<int>(std::floor(wound.accumulator)));
        if (count > 0) {
            wound.accumulator -= static_cast<double>(count);
            const double spray = std::clamp((wound.pressure - materials_.woundSprayPressure) / 2.4, 0.0, 1.0) *
                                 std::clamp(1.0 - wound.age / 0.9, 0.0, 1.0);
            const Vec2 leakDirection = normalized({
                                                     wound.direction.x * (0.25 + spray * 0.85),
                                                     wound.direction.y * (0.25 + spray * 0.85) + 0.85 - spray * 0.30,
                                                 },
                                                 {0.0, 1.0});
            const int emittedBefore = stats_.emittedFluidParticles;
            emitFluid(wound.position,
                      leakDirection,
                      count,
                      45.0 + wound.pressure * (38.0 + spray * 92.0),
                      wound.radius * (0.64 + wound.depth * 0.18),
                      0.58 + wound.depth * 0.42 + spray * 0.18);
            const int emitted = stats_.emittedFluidParticles - emittedBefore;
            stats_.woundFluidParticles += emitted;
            debug_.woundLeaks += emitted;
        }

        wound.pressure = std::max(0.0, wound.pressure - dt * materials_.woundPressureDecay * (0.36 + wound.clot));
        wound.clot = std::min(1.0, wound.clot + dt * materials_.woundClotRate *
                                         (wound.layer == TissueLayer::Skin ? 1.16 : 0.72) *
                                         (0.82 + 0.32 / std::max(0.35, wound.pressure)));
        if (wound.pressure < 0.055 || wound.clot > 0.985) {
            wound.active = false;
            wound.accumulator = 0.0;
        } else {
            ++debug_.activeWounds;
        }
    }
}

void World::damageTissueAroundFracture(Vec2 center, double radius, double impulse) {
    const double radiusSq = radius * radius;
    const double skinRadius = radius * 0.62;

    for (Spring& spring : springs_) {
        if (spring.broken) {
            continue;
        }

        Point& a = points_[spring.a];
        Point& b = points_[spring.b];
        const double d = distanceToSegment(center, a.position, b.position);
        if (spring.layer == TissueLayer::Muscle && d <= radius) {
            const Vec2 midpoint{(a.position.x + b.position.x) * 0.5, (a.position.y + b.position.y) * 0.5};
            spring.broken = true;
            spring.stress = 1.0;
            a.exposure = std::max(a.exposure, 1.0);
            b.exposure = std::max(b.exposure, 1.0);
            a.load = std::max(a.load, impulse * 0.30);
            b.load = std::max(b.load, impulse * 0.30);
            ++stats_.brokenMuscle;
            emitFluid(midpoint,
                      {midpoint.x - center.x, midpoint.y - center.y - radius * 0.15},
                      3 + static_cast<int>(std::clamp(impulse / 900.0, 0.0, 7.0)),
                      90.0 + impulse * materials_.fluidImpactScale,
                      2.2,
                      1.05);
            openWound(midpoint,
                      {midpoint.x - center.x, midpoint.y - center.y - radius * 0.15},
                      TissueLayer::Muscle,
                      impulse / 1050.0,
                      2.2,
                      1.0);
        } else if (spring.layer == TissueLayer::Skin && d <= skinRadius && impulse > materials_.skinTearImpulse * 1.18) {
            const Vec2 midpoint{(a.position.x + b.position.x) * 0.5, (a.position.y + b.position.y) * 0.5};
            spring.broken = true;
            spring.stress = 1.0;
            a.exposure = std::max(a.exposure, 1.0);
            b.exposure = std::max(b.exposure, 1.0);
            a.load = std::max(a.load, impulse * 0.18);
            b.load = std::max(b.load, impulse * 0.18);
            ++stats_.brokenSkin;
            emitFluid(midpoint,
                      {midpoint.x - center.x, midpoint.y - center.y - skinRadius * 0.25},
                      5 + static_cast<int>(std::clamp(impulse / 780.0, 0.0, 9.0)),
                      120.0 + impulse * materials_.fluidImpactScale,
                      2.4,
                      1.18);
            openWound(midpoint,
                      {midpoint.x - center.x, midpoint.y - center.y - skinRadius * 0.25},
                      TissueLayer::Skin,
                      impulse / 1250.0,
                      2.4,
                      0.68);
        }
    }

    for (Attachment& attachment : attachments_) {
        if (attachment.broken) {
            continue;
        }
        Point& skin = points_[attachment.skinPoint];
        Point& muscle = points_[attachment.musclePoint];
        const double skinDistanceSq = (skin.position.x - center.x) * (skin.position.x - center.x) +
                                      (skin.position.y - center.y) * (skin.position.y - center.y);
        const double muscleDistanceSq = (muscle.position.x - center.x) * (muscle.position.x - center.x) +
                                        (muscle.position.y - center.y) * (muscle.position.y - center.y);
        if (std::min(skinDistanceSq, muscleDistanceSq) <= radiusSq) {
            const Vec2 midpoint{(skin.position.x + muscle.position.x) * 0.5, (skin.position.y + muscle.position.y) * 0.5};
            attachment.broken = true;
            skin.exposure = std::max(skin.exposure, 1.0);
            muscle.exposure = std::max(muscle.exposure, 1.0);
            ++stats_.brokenAttachments;
            emitFluid(midpoint,
                      {midpoint.x - center.x, midpoint.y - center.y - radius * 0.10},
                      2 + static_cast<int>(std::clamp(impulse / 1200.0, 0.0, 5.0)),
                      75.0 + impulse * materials_.fluidImpactScale * 0.55,
                      1.8,
                      0.78);
            openWound(midpoint,
                      {midpoint.x - center.x, midpoint.y - center.y - radius * 0.10},
                      TissueLayer::Muscle,
                      impulse / 1500.0,
                      1.8,
                      0.74);
        }
    }

    for (Triangle& triangle : triangles_) {
        if (triangle.failed || triangle.layer != TissueLayer::Muscle) {
            continue;
        }
        Point& a = points_[triangle.a];
        Point& b = points_[triangle.b];
        Point& c = points_[triangle.c];
        const Vec2 centroid{(a.position.x + b.position.x + c.position.x) / 3.0,
                            (a.position.y + b.position.y + c.position.y) / 3.0};
        const double dx = centroid.x - center.x;
        const double dy = centroid.y - center.y;
        if (dx * dx + dy * dy <= radiusSq) {
            triangle.damage = std::max(triangle.damage, 1.08);
            triangle.failed = true;
            a.exposure = std::max(a.exposure, 1.0);
            b.exposure = std::max(b.exposure, 1.0);
            c.exposure = std::max(c.exposure, 1.0);
            emitFluid(centroid,
                      {centroid.x - center.x, centroid.y - center.y - radius * 0.08},
                      2 + static_cast<int>(std::clamp(impulse / 1600.0, 0.0, 4.0)),
                      70.0 + impulse * materials_.fluidImpactScale * 0.45,
                      1.7,
                      0.72);
            openWound(centroid,
                      {centroid.x - center.x, centroid.y - center.y - radius * 0.08},
                      TissueLayer::Muscle,
                      impulse / 1750.0,
                      1.7,
                      0.86);
        }
    }
}

void World::fractureBone(std::size_t boneIndex, double fractureT, Vec2 impulseNormal, double impulse) {
    if (boneIndex >= bones_.size() || !canFractureBone(bones_[boneIndex])) {
        return;
    }

    BoneSegment& bone = bones_[boneIndex];
    const Vec2 oldA = bone.a;
    const Vec2 oldB = bone.b;
    const Vec2 oldPreviousA = bone.previousA;
    const Vec2 oldPreviousB = bone.previousB;
    const Vec2 oldHomeA = bone.homeA;
    const Vec2 oldHomeB = bone.homeB;
    const double oldRadius = bone.radius;
    const double oldLoad = bone.load;
    const double oldFractureImpulse = bone.fractureImpulse;
    const bool oldPinned = bone.pinned;
    const double dx = oldB.x - oldA.x;
    const double dy = oldB.y - oldA.y;
    const double len = std::max(kEpsilon, std::sqrt(dx * dx + dy * dy));
    const double minimumPieceLength = std::max(materials_.minBoneFragmentLength, oldRadius * 3.2);
    const double minBreakT = std::clamp(minimumPieceLength / len, 0.18, 0.46);
    if (minBreakT >= 0.5) {
        return;
    }
    const Vec2 dir{dx / len, dy / len};
    const Vec2 baseNormal{-dir.y, dir.x};
    const Vec2 contactNormal = normalized(impulseNormal, baseNormal);
    const Vec2 normal = normalized({baseNormal.x * 0.35 + contactNormal.x * 0.65,
                                    baseNormal.y * 0.35 + contactNormal.y * 0.65},
                                   baseNormal);

    const double breakT = std::clamp(fractureT, minBreakT, 1.0 - minBreakT);
    const Vec2 crack = lerp(oldA, oldB, breakT);
    const Vec2 previousCrack = lerp(oldPreviousA, oldPreviousB, breakT);
    const Vec2 homeCrack = lerp(oldHomeA, oldHomeB, breakT);
    const double overload = std::clamp((std::max(impulse, oldLoad) - oldFractureImpulse) / std::max(1.0, oldFractureImpulse), 0.0, 1.4);
    const double gap = std::min(std::max(5.0, oldRadius * (0.75 + overload * 0.25)), len * 0.10);
    const double snap = std::min(std::max(5.5, oldRadius * (1.05 + overload * 0.42)), len * 0.09);
    const double shear = std::min(std::max(2.5, oldRadius * 0.42), len * 0.035);
    const double recoil = snap * (2.1 + overload * 0.8);
    const Vec2 leftCap{crack.x - dir.x * gap * 0.5 - normal.x * snap - dir.x * shear,
                       crack.y - dir.y * gap * 0.5 - normal.y * snap - dir.y * shear};
    const Vec2 rightCap{crack.x + dir.x * gap * 0.5 + normal.x * snap + dir.x * shear,
                        crack.y + dir.y * gap * 0.5 + normal.y * snap + dir.y * shear};

    bone.a = {oldA.x - normal.x * snap * 0.18, oldA.y - normal.y * snap * 0.18};
    bone.b = leftCap;
    bone.previousA = {oldPreviousA.x + normal.x * recoil * 0.20, oldPreviousA.y + normal.y * recoil * 0.20};
    bone.previousB = {previousCrack.x + normal.x * recoil + dir.x * shear * 0.7,
                      previousCrack.y + normal.y * recoil + dir.y * shear * 0.7};
    bone.homeB = {homeCrack.x - dir.x * gap * 0.5 - normal.x * snap * 0.35,
                  homeCrack.y - dir.y * gap * 0.5 - normal.y * snap * 0.35};
    bone.restLength = std::max(kEpsilon, distance(bone.a, bone.b));
    bone.fractured = true;
    bone.brokenEnd = true;
    bone.brokenEndNormal = normal;
    ++bone.fractureGeneration;
    bone.fractureImpulse = oldFractureImpulse * 0.82;
    bone.load = oldLoad * 0.28;
    const double spinSign = cross(dir, normal) >= 0.0 ? 1.0 : -1.0;
    const double fractureSpin = std::clamp((std::max(impulse, oldLoad) / std::max(1.0, oldFractureImpulse)) *
                                               materials_.fractureSpinScale *
                                               (0.55 + std::abs(breakT - 0.5) * 1.9),
                                           0.0,
                                           16.0);
    bone.angularVelocity = std::clamp(bone.angularVelocity + spinSign * fractureSpin * (1.0 - breakT),
                                      -36.0,
                                      36.0);
    const double firstAngularVelocity = bone.angularVelocity;

    BoneSegment second;
    second.a = rightCap;
    second.b = {oldB.x + normal.x * snap * 0.18, oldB.y + normal.y * snap * 0.18};
    second.previousA = {previousCrack.x - normal.x * recoil - dir.x * shear * 0.7,
                        previousCrack.y - normal.y * recoil - dir.y * shear * 0.7};
    second.previousB = {oldPreviousB.x - normal.x * recoil * 0.20, oldPreviousB.y - normal.y * recoil * 0.20};
    second.homeA = {homeCrack.x + dir.x * gap * 0.5 + normal.x * snap * 0.35,
                    homeCrack.y + dir.y * gap * 0.5 + normal.y * snap * 0.35};
    second.homeB = oldHomeB;
    second.radius = oldRadius;
    second.restLength = std::max(kEpsilon, distance(second.a, second.b));
    second.fractureImpulse = oldFractureImpulse * 0.82;
    second.load = oldLoad * 0.28;
    second.fractured = true;
    second.brokenStart = true;
    second.brokenStartNormal = normal;
    second.fractureGeneration = bone.fractureGeneration;
    second.pinned = oldPinned;
    second.angularVelocity = std::clamp(bone.angularVelocity - spinSign * fractureSpin * (0.65 + breakT),
                                        -36.0,
                                        36.0);
    const double secondAngularVelocity = second.angularVelocity;
    const std::size_t secondIndex = bones_.size();
    bones_.push_back(second);

    for (BoneAttachment& attachment : boneAttachments_) {
        if (attachment.bone != boneIndex || attachment.broken) {
            continue;
        }
        const double originalT = attachment.t;
        const double detachZone = std::clamp(0.11 + overload * 0.05 + oldRadius / std::max(len, 1.0) * 1.5, 0.10, 0.22);
        if (std::abs(originalT - breakT) <= detachZone) {
            attachment.broken = true;
            attachment.stress = 1.0 + overload;
            points_[attachment.point].exposure = std::max(points_[attachment.point].exposure, 0.95);
            points_[attachment.point].load = std::max(points_[attachment.point].load, oldLoad * 0.35);
            ++stats_.brokenBoneAttachments;
            continue;
        }
        if (originalT < breakT) {
            attachment.t = std::clamp(originalT / std::max(0.05, breakT), 0.0, 1.0);
            const Vec2 anchor = bonePoint(bones_[boneIndex], attachment.t);
            attachment.offset = {points_[attachment.point].position.x - anchor.x, points_[attachment.point].position.y - anchor.y};
            attachment.rest = std::max(1.0, distance(points_[attachment.point].position, anchor));
        } else {
            attachment.bone = secondIndex;
            attachment.t = std::clamp((originalT - breakT) / std::max(0.05, 1.0 - breakT), 0.0, 1.0);
            const Vec2 anchor = bonePoint(bones_[secondIndex], attachment.t);
            attachment.offset = {points_[attachment.point].position.x - anchor.x, points_[attachment.point].position.y - anchor.y};
            attachment.rest = std::max(1.0, distance(points_[attachment.point].position, anchor));
        }
    }

    for (BoneJoint& joint : boneJoints_) {
        if (joint.broken) {
            continue;
        }

        bool jointAffected = false;
        auto remapJointEnd = [&](std::size_t& jointBone, double& jointT) {
            if (jointBone != boneIndex) {
                return;
            }
            jointAffected = true;
            const double originalT = jointT;
            const double detachZone = std::clamp(0.05 + overload * 0.03 + oldRadius / std::max(len, 1.0), 0.05, 0.14);
            if (std::abs(originalT - breakT) <= detachZone) {
                joint.broken = true;
                joint.stress = 1.0 + overload;
                ++stats_.brokenBoneJoints;
                return;
            }
            if (originalT < breakT) {
                jointT = std::clamp(originalT / std::max(0.05, breakT), 0.0, 1.0);
            } else {
                jointBone = secondIndex;
                jointT = std::clamp((originalT - breakT) / std::max(0.05, 1.0 - breakT), 0.0, 1.0);
            }
        };

        remapJointEnd(joint.a, joint.tA);
        remapJointEnd(joint.b, joint.tB);
        if (jointAffected && joint.a < bones_.size() && joint.b < bones_.size()) {
            const double remappedRest = std::max(1.0, distance(bonePoint(bones_[joint.a], joint.tA), bonePoint(bones_[joint.b], joint.tB)));
            const double remappedRestAngle = wrapAngle(boneAngle(bones_[joint.b]) - boneAngle(bones_[joint.a]));
            joint.postFractureLimited = true;
            joint.postFractureRest = remappedRest;
            joint.postFractureRestAngle = remappedRestAngle;
            joint.torqueStress = 0.0;
            if (!joint.broken) {
                joint.rest = remappedRest;
                joint.restAngle = remappedRestAngle;
            }
        }
    }

    BoneSegment splinter;
    const double chipLength = std::min(std::max(oldRadius * 1.8, 8.0), len * 0.13);
    splinter.a = {crack.x - dir.x * chipLength * 0.45 + normal.x * snap * 0.35,
                  crack.y - dir.y * chipLength * 0.45 + normal.y * snap * 0.35};
    splinter.b = {crack.x + dir.x * chipLength * 0.55 + normal.x * snap * 1.35,
                  crack.y + dir.y * chipLength * 0.55 + normal.y * snap * 1.35};
    splinter.previousA = {splinter.a.x - normal.x * recoil * 0.55, splinter.a.y - normal.y * recoil * 0.55};
    splinter.previousB = {splinter.b.x - normal.x * recoil * 0.55, splinter.b.y - normal.y * recoil * 0.55};
    splinter.homeA = {homeCrack.x - dir.x * chipLength * 0.45 + normal.x * snap * 0.2,
                      homeCrack.y - dir.y * chipLength * 0.45 + normal.y * snap * 0.2};
    splinter.homeB = {homeCrack.x + dir.x * chipLength * 0.55 + normal.x * snap * 0.2,
                      homeCrack.y + dir.y * chipLength * 0.55 + normal.y * snap * 0.2};
    splinter.radius = std::max(2.5, oldRadius * 0.42);
    splinter.restLength = std::max(kEpsilon, distance(splinter.a, splinter.b));
    splinter.fractureImpulse = oldFractureImpulse;
    splinter.load = oldLoad * 0.5;
    splinter.fractured = true;
    splinter.brokenStart = true;
    splinter.brokenEnd = true;
    splinter.brokenStartNormal = normal;
    splinter.brokenEndNormal = normal;
    splinter.fractureGeneration = materials_.maxBoneFractureDepth;
    splinter.splinter = true;
    splinter.angularVelocity = std::clamp(spinSign * fractureSpin * 1.65, -42.0, 42.0);
    const double splinterAngularVelocity = splinter.angularVelocity;
    bones_.push_back(splinter);
    debug_.maxBoneAngularSpeed = std::max(debug_.maxBoneAngularSpeed,
                                          std::max(std::abs(firstAngularVelocity),
                                                   std::max(std::abs(secondAngularVelocity), std::abs(splinterAngularVelocity))));

    emitFluid(crack,
              {normal.x + dir.x * 0.28, normal.y + dir.y * 0.28 - 0.30},
              10 + static_cast<int>(std::clamp(std::max(impulse, oldLoad) / 720.0, 0.0, 16.0)),
              180.0 + std::max(impulse, oldLoad) * materials_.fluidImpactScale,
              2.7,
              1.28);
    openWound(crack,
              {normal.x + dir.x * 0.28, normal.y + dir.y * 0.28 - 0.30},
              TissueLayer::Muscle,
              std::max(impulse, oldLoad) / 980.0,
              2.9,
              1.18);
    damageTissueAroundFracture(crack, std::max(oldRadius * 3.8, 24.0 + overload * 10.0), std::max(impulse, oldLoad));

    ++stats_.fracturedBones;
    ++debug_.fractures;
    debug_.lastFractureImpulse = std::max(debug_.lastFractureImpulse, std::max(impulse, oldLoad));
}

void World::integrate(double dt, double width, double floorY) {
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
        const double shapeStiffness = bone.fractured ? 0.0 : materials_.boneShapeStiffness;
        bone.angularVelocity *= materials_.boneAngularDamping;
        const bool freeFragment = bone.fractured || bone.splinter;
        if (!freeFragment && std::abs(bone.angularVelocity) < 0.01) {
            bone.angularVelocity = 0.0;
        }
        if (freeFragment) {
            debug_.maxBoneAngularSpeed = std::max(debug_.maxBoneAngularSpeed, std::abs(bone.angularVelocity));
        }
        bone.previousA = bone.a;
        bone.previousB = bone.b;
        bone.a.x += avx + (bone.homeA.x - bone.a.x) * shapeStiffness;
        bone.a.y += avy + materials_.gravity * dt * dt + (bone.homeA.y - bone.a.y) * shapeStiffness;
        bone.b.x += bvx + (bone.homeB.x - bone.b.x) * shapeStiffness;
        bone.b.y += bvy + materials_.gravity * dt * dt + (bone.homeB.y - bone.b.y) * shapeStiffness;
        if (freeFragment) {
            rotateBoneAroundCenter(bone, bone.angularVelocity * dt);
        }
    }

    for (FluidParticle& fluid : fluids_) {
        if (fluid.life <= 0.0) {
            continue;
        }

        fluid.life = std::max(0.0, fluid.life - dt);
        if (fluid.life <= 0.0) {
            continue;
        }

        if (fluid.settled) {
            fluid.life = std::max(0.0, fluid.life - dt * 0.45);
            continue;
        }

        const double vx = (fluid.position.x - fluid.previous.x) * materials_.fluidDamping;
        const double vy = (fluid.position.y - fluid.previous.y) * materials_.fluidDamping;
        fluid.previous = fluid.position;
        fluid.position.x += vx;
        fluid.position.y += vy + materials_.gravity * materials_.fluidGravityScale * dt * dt;

        const double margin = fluid.radius + 1.0;
        if (fluid.position.x < margin) {
            fluid.position.x = margin;
            fluid.previous.x = fluid.position.x + vx * materials_.fluidFloorFriction;
        } else if (fluid.position.x > width - margin) {
            fluid.position.x = width - margin;
            fluid.previous.x = fluid.position.x + vx * materials_.fluidFloorFriction;
        }
        if (fluid.position.y > floorY - fluid.radius) {
            fluid.position.y = floorY - fluid.radius;
            fluid.previous.x = fluid.position.x + vx * materials_.fluidFloorFriction;
            fluid.previous.y = fluid.position.y + vy * materials_.fluidFloorFriction;
            if (std::abs(vx) + std::abs(vy) < 1.2) {
                fluid.settled = true;
            }
        }
    }
}

void World::collideStriker(double dt, const InputState& input) {
    if (!input.active) {
        return;
    }

    const ToolProfile profile = toolProfile(input.tool);
    const double speed = std::sqrt(input.vx * input.vx + input.vy * input.vy);
    const double radius = materials_.strikerRadius * profile.radiusScale;
    const double impact = speed * materials_.strikerMass * input.power * profile.massScale;
    const ToolContactShape shape = makeToolContactShape(input, profile, radius);
    const double influence = shape.influence;

    const std::size_t initialBoneCount = bones_.size();
    for (std::size_t i = 0; i < initialBoneCount; ++i) {
        BoneSegment& bone = bones_[i];
        bone.load *= 0.88;
        double t = segmentT(shape.center, bone.a, bone.b);
        Vec2 closest = bonePoint(bone, t);
        Vec2 toolContact = shape.center;
        double dist = distance(closest, toolContact);
        if (shape.bladeSegment) {
            const SegmentClosestPoints closestPair = closestSegmentPoints(shape.axisStart, shape.axisEnd, bone.a, bone.b);
            t = closestPair.tB;
            closest = closestPair.pointB;
            toolContact = closestPair.pointA;
            dist = closestPair.distance;
        }
        if (!input.down || dist > influence + bone.radius) {
            continue;
        }

        Vec2 normal = normalized(subtract(closest, toolContact), shape.bladeSegment ? shape.bladeNormal : shape.direction);
        if (dist < kEpsilon && !shape.bladeSegment && speed > kEpsilon) {
            normal = shape.direction;
            dist = 1.0;
        }

        const double nx = normal.x;
        const double ny = normal.y;
        const double depth = std::max(0.0, influence + bone.radius - dist);
        const double contact = 1.0 - std::clamp((dist - bone.radius) / influence, 0.0, 1.0);
        const double directLoad = (impact + materials_.boneDirectPressure * input.power) * contact * profile.boneLoadScale;
        bone.load = std::max(bone.load, directLoad);
        ++debug_.boneContacts;
        if (depth > debug_.maxDepth) {
            debug_.maxDepth = depth;
            debug_.strongestContact = closest;
        }
        debug_.maxBoneLoad = std::max(debug_.maxBoneLoad, bone.load);

        if (!bone.pinned) {
            const double contactStrength = materials_.boneDirectContact * profile.bonePushScale * (0.70 + input.power * 0.12);
            const double pushX = nx * depth * contactStrength * profile.reboundScale +
                                 input.vx * dt * contactStrength * 0.58 * profile.dragScale;
            const double pushY = ny * depth * contactStrength * profile.reboundScale +
                                 input.vy * dt * contactStrength * 0.58 * profile.dragScale;
            const double aWeight = 1.0 - t;
            const double bWeight = t;
            bone.a.x += pushX * aWeight;
            bone.a.y += pushY * aWeight;
            bone.b.x += pushX * bWeight;
            bone.b.y += pushY * bWeight;
            applyBoneTorque(bone,
                            closest,
                            {nx * directLoad * 0.16 * profile.reboundScale +
                                 input.vx * contact * profile.boneLoadScale * 0.22 * profile.dragScale,
                             ny * directLoad * 0.16 * profile.reboundScale +
                                 input.vy * contact * profile.boneLoadScale * 0.22 * profile.dragScale});
        }

        if (canFractureBone(bone) && bone.load > bone.fractureImpulse * profile.fractureScale) {
            fractureBone(i, t, {nx, ny}, directLoad);
        }
    }

    for (Point& point : points_) {
        if (point.pinned) {
            continue;
        }

        const ToolPointContact pointContact = samplePointContact(point.position, shape);
        if (pointContact.distance > influence) {
            continue;
        }

        double contactStrength = (input.down ? 0.74 : 0.20) * profile.tissuePushScale * (0.85 + input.power * 0.15);
        if (point.layer == TissueLayer::Muscle) {
            contactStrength *= materials_.directMuscleContact + point.exposure * 0.82;
        }

        const double nx = pointContact.normal.x;
        const double ny = pointContact.normal.y;
        const double depth = influence - pointContact.distance;
        point.position.x += nx * depth * contactStrength * profile.reboundScale +
                            input.vx * dt * 0.45 * contactStrength * profile.dragScale;
        point.position.y += ny * depth * contactStrength * profile.reboundScale +
                            input.vy * dt * 0.45 * contactStrength * profile.dragScale;
        point.load = std::max(point.load, impact * (depth / influence) * contactStrength * profile.tissueLoadScale);
        ++debug_.tissueContacts;
        if (depth > debug_.maxDepth) {
            debug_.maxDepth = depth;
            debug_.strongestContact = point.position;
        }
        debug_.maxPointLoad = std::max(debug_.maxPointLoad, point.load);
    }

    if (input.down && profile.tearPressureScale > 0.0) {
        for (Spring& spring : springs_) {
            if (spring.broken) {
                continue;
            }

            Point& a = points_[spring.a];
            Point& b = points_[spring.b];
            const Vec2 midpoint{(a.position.x + b.position.x) * 0.5, (a.position.y + b.position.y) * 0.5};
            const ToolPointContact tearContact = samplePointContact(midpoint, shape);
            const double d = tearContact.distance;
            if (d > influence * 0.82) {
                continue;
            }

            const double contact = 1.0 - std::clamp(d / std::max(1.0, influence * 0.82), 0.0, 1.0);
            const double layerScale = spring.layer == TissueLayer::Skin ? 1.0 : 0.78 + std::max(a.exposure, b.exposure) * 0.42;
            const double pressure = impact * profile.tearPressureScale * contact * layerScale;
            const double threshold = spring.tearImpulse * materials_.sharpToolTearPressure;
            spring.stress = std::max(spring.stress, pressure / std::max(1.0, threshold));
            if (pressure <= threshold) {
                continue;
            }

            const Vec2 tangent = normalized({b.position.x - a.position.x, b.position.y - a.position.y}, {1.0, 0.0});
            Vec2 cutNormal = shape.bladeNormal;
            const Vec2 springNormal = normalized({-tangent.y, tangent.x - 0.25}, {-tangent.y, tangent.x});
            if (dot(cutNormal, springNormal) < 0.0) {
                cutNormal = scale(cutNormal, -1.0);
            }
            const Vec2 normal = normalized({
                                            springNormal.x * (1.0 - profile.bladeNormalBias) + cutNormal.x * profile.bladeNormalBias,
                                            springNormal.y * (1.0 - profile.bladeNormalBias) + cutNormal.y * profile.bladeNormalBias,
                                        },
                                        springNormal);
            spring.broken = true;
            a.exposure = std::max(a.exposure, spring.layer == TissueLayer::Skin ? 0.92 : 1.0);
            b.exposure = std::max(b.exposure, spring.layer == TissueLayer::Skin ? 0.92 : 1.0);
            a.load = std::max(a.load, pressure * 0.18);
            b.load = std::max(b.load, pressure * 0.18);
            if (spring.layer == TissueLayer::Skin) {
                ++stats_.brokenSkin;
            } else {
                ++stats_.brokenMuscle;
            }
            emitFluid(midpoint,
                      normal,
                      spring.layer == TissueLayer::Skin ? 6 : 4,
                      120.0 + pressure * materials_.fluidImpactScale * 0.42,
                      spring.layer == TissueLayer::Skin ? 2.1 : 1.8,
                      profile.fluidScale);
            openWound(midpoint,
                      normal,
                      spring.layer,
                      pressure / (spring.layer == TissueLayer::Skin ? 1250.0 : 1050.0),
                      spring.layer == TissueLayer::Skin ? 2.1 : 1.8,
                      spring.layer == TissueLayer::Skin ? 0.58 : 0.92);
        }
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
            const Vec2 midpoint{(a.position.x + b.position.x) * 0.5, (a.position.y + b.position.y) * 0.5};
            const Vec2 tangent = normalized({b.position.x - a.position.x, b.position.y - a.position.y}, {1.0, 0.0});
            const Vec2 normal{-tangent.y, tangent.x - 0.35};
            spring.broken = true;
            if (spring.layer == TissueLayer::Skin) {
                ++stats_.brokenSkin;
                emitFluid(midpoint,
                          normal,
                          5 + static_cast<int>(std::clamp(endpointLoad / 900.0, 0.0, 8.0)),
                          110.0 + endpointLoad * materials_.fluidImpactScale,
                          2.3,
                          1.12);
                openWound(midpoint,
                          normal,
                          TissueLayer::Skin,
                          endpointLoad / 1350.0,
                          2.3,
                          0.55);
            } else {
                ++stats_.brokenMuscle;
                emitFluid(midpoint,
                          normal,
                          3 + static_cast<int>(std::clamp(endpointLoad / 1050.0, 0.0, 6.0)),
                          85.0 + endpointLoad * materials_.fluidImpactScale * 0.75,
                          1.9,
                          0.86);
                openWound(midpoint,
                          normal,
                          TissueLayer::Muscle,
                          endpointLoad / 1250.0,
                          1.9,
                          0.86);
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
            const Vec2 midpoint{(skin.position.x + muscle.position.x) * 0.5, (skin.position.y + muscle.position.y) * 0.5};
            attachment.broken = true;
            ++stats_.brokenAttachments;
            skin.exposure = std::max(skin.exposure, 1.0);
            muscle.exposure = std::max(muscle.exposure, 1.0);
            emitFluid(midpoint,
                      {skin.position.x - muscle.position.x, skin.position.y - muscle.position.y - 0.4},
                      3 + static_cast<int>(std::clamp(impulse / 1200.0, 0.0, 5.0)),
                      80.0 + impulse * materials_.fluidImpactScale * 0.60,
                      1.8,
                      0.78);
            openWound(midpoint,
                      {skin.position.x - muscle.position.x, skin.position.y - muscle.position.y - 0.4},
                      TissueLayer::Muscle,
                      impulse / 1550.0,
                      1.8,
                      0.74);
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
        bone.load = std::max(bone.load, point.load * materials_.boneImpactTransfer);
        const Vec2 rawAnchor = bonePoint(bone, attachment.t);
        const double currentBoneDistance = distance(point.position, rawAnchor);
        const double stretchRatio = currentBoneDistance / std::max(1.0, attachment.rest);
        const double impulse = std::max(point.load, bone.load);
        attachment.stress = std::max(attachment.stress * 0.9, std::max(0.0, stretchRatio - 1.0));

        if (stretchRatio > materials_.boneAttachmentBreakStretch || (impulse > materials_.boneAttachmentBreakImpulse && stretchRatio > 1.45)) {
            const Vec2 midpoint{(point.position.x + rawAnchor.x) * 0.5, (point.position.y + rawAnchor.y) * 0.5};
            attachment.broken = true;
            ++stats_.brokenBoneAttachments;
            point.exposure = std::max(point.exposure, 0.85);
            emitFluid(midpoint,
                      {point.position.x - rawAnchor.x, point.position.y - rawAnchor.y - 0.35},
                      4 + static_cast<int>(std::clamp(impulse / 1050.0, 0.0, 7.0)),
                      95.0 + impulse * materials_.fluidImpactScale * 0.72,
                      2.0,
                      0.94);
            openWound(midpoint,
                      {point.position.x - rawAnchor.x, point.position.y - rawAnchor.y - 0.35},
                      TissueLayer::Muscle,
                      impulse / 1250.0,
                      2.0,
                      0.98);
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

void World::solveBoneJoints() {
    for (BoneJoint& joint : boneJoints_) {
        if (joint.broken || joint.postFractureLimited || joint.a >= bones_.size() || joint.b >= bones_.size()) {
            continue;
        }

        BoneSegment& a = bones_[joint.a];
        BoneSegment& b = bones_[joint.b];
        const double loadA = a.load;
        const double loadB = b.load;
        const Vec2 anchorA = bonePoint(a, joint.tA);
        const Vec2 anchorB = bonePoint(b, joint.tB);
        const double dx = anchorB.x - anchorA.x;
        const double dy = anchorB.y - anchorA.y;
        const double len = std::sqrt(dx * dx + dy * dy);
        if (len < kEpsilon) {
            continue;
        }

        const double stretchRatio = len / std::max(1.0, joint.rest);
        const double impulse = std::max(loadA, loadB);
        joint.stress = std::max(joint.stress * 0.9, std::max(0.0, stretchRatio - 1.0));

        if (stretchRatio > materials_.boneJointBreakStretch || (impulse > materials_.boneJointBreakImpulse && stretchRatio > 1.35)) {
            joint.broken = true;
            ++stats_.brokenBoneJoints;
            continue;
        }

        const double relativeAngle = wrapAngle(boneAngle(b) - boneAngle(a) - joint.restAngle);
        const double clampedAngle = std::clamp(relativeAngle, joint.minAngle, joint.maxAngle);
        const double angleViolation = relativeAngle - clampedAngle;
        const double overextension = std::abs(angleViolation);
        joint.torqueStress = std::max(joint.torqueStress * 0.9, overextension);

        if (overextension > materials_.boneJointAngularBreak ||
            (impulse > materials_.boneJointBreakImpulse && overextension > materials_.boneJointAngularBreak * 0.45)) {
            joint.broken = true;
            ++stats_.brokenBoneJoints;
            continue;
        }

        a.load = std::max(loadA, loadB * 0.30);
        b.load = std::max(loadB, loadA * 0.30);

        const double diff = (len - joint.rest) / len;
        const double correctionX = dx * diff * materials_.boneJointStiffness * 0.5;
        const double correctionY = dy * diff * materials_.boneJointStiffness * 0.5;
        applyBoneAnchorDelta(a, joint.tA, correctionX, correctionY);
        applyBoneAnchorDelta(b, joint.tB, -correctionX, -correctionY);

        const double angleCorrection = angleViolation * materials_.boneJointAngularStiffness * 0.5;
        rotateBoneAroundAnchor(a, joint.tA, angleCorrection);
        rotateBoneAroundAnchor(b, joint.tB, -angleCorrection);
    }
}

void World::solveBones() {
    const std::size_t initialBoneCount = bones_.size();
    for (std::size_t i = 0; i < initialBoneCount; ++i) {
        BoneSegment& bone = bones_[i];
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

        if (canFractureBone(bone) && bone.load > bone.fractureImpulse) {
            fractureBone(i);
        }
    }
}

void World::solvePostFractureJoints() {
    for (BoneJoint& joint : boneJoints_) {
        if ((!joint.broken && !joint.postFractureLimited) || joint.a >= bones_.size() || joint.b >= bones_.size()) {
            continue;
        }

        BoneSegment& a = bones_[joint.a];
        BoneSegment& b = bones_[joint.b];
        if (a.splinter || b.splinter || (a.pinned && b.pinned)) {
            continue;
        }

        const double invMassA = a.pinned ? 0.0 : 1.0 / std::max(1.0, a.restLength * a.radius * a.radius);
        const double invMassB = b.pinned ? 0.0 : 1.0 / std::max(1.0, b.restLength * b.radius * b.radius);
        const double invMassSum = invMassA + invMassB;
        if (invMassSum <= kEpsilon) {
            continue;
        }
        const double shareA = invMassA / invMassSum;
        const double shareB = invMassB / invMassSum;

        bool corrected = false;
        const double rest = std::max(1.0, joint.postFractureRest > 0.0 ? joint.postFractureRest : joint.rest);
        const double restAngle = joint.postFractureRest > 0.0 ? joint.postFractureRestAngle : joint.restAngle;
        const Vec2 anchorA = bonePoint(a, joint.tA);
        const Vec2 anchorB = bonePoint(b, joint.tB);
        const double dx = anchorB.x - anchorA.x;
        const double dy = anchorB.y - anchorA.y;
        const double len = std::sqrt(dx * dx + dy * dy);
        if (len > kEpsilon) {
            const double maxLen = std::max(rest + materials_.postFractureJointSlack,
                                           rest * materials_.postFractureJointMaxStretch);
            const double stretchRatio = len / rest;
            debug_.maxPostFractureJointStretch = std::max(debug_.maxPostFractureJointStretch, stretchRatio);
            joint.stress = std::max(joint.stress * 0.94, std::max(0.0, stretchRatio - 1.0));
            if (len > maxLen) {
                const double diff = (len - maxLen) / len;
                const double correctionX = dx * diff * materials_.postFractureJointStiffness;
                const double correctionY = dy * diff * materials_.postFractureJointStiffness;
                applyBoneAnchorDelta(a, joint.tA, correctionX * shareA, correctionY * shareA);
                applyBoneAnchorDelta(b, joint.tB, -correctionX * shareB, -correctionY * shareB);
                corrected = true;
            }
        }

        const double relativeAngle = wrapAngle(boneAngle(b) - boneAngle(a) - restAngle);
        const double minAngle = joint.minAngle - materials_.postFractureJointAngleSlack;
        const double maxAngle = joint.maxAngle + materials_.postFractureJointAngleSlack;
        const double clampedAngle = std::clamp(relativeAngle, minAngle, maxAngle);
        const double angleViolation = relativeAngle - clampedAngle;
        const double overextension = std::abs(angleViolation);
        debug_.maxPostFractureJointAngle = std::max(debug_.maxPostFractureJointAngle, std::abs(relativeAngle));
        joint.torqueStress = std::max(joint.torqueStress * 0.94, overextension);
        if (overextension > kEpsilon) {
            const double correction = angleViolation * materials_.postFractureJointAngularStiffness;
            rotateBoneAroundAnchor(a, joint.tA, correction * shareA);
            rotateBoneAroundAnchor(b, joint.tB, -correction * shareB);
            a.angularVelocity *= 1.0 - 0.08 * shareA;
            b.angularVelocity *= 1.0 - 0.08 * shareB;
            corrected = true;
        }

        if (corrected) {
            ++debug_.postFractureJointCorrections;
        }
    }
}

void World::solveBoneFragmentRepulsion() {
    const std::size_t count = bones_.size();
    for (std::size_t i = 0; i < count; ++i) {
        BoneSegment& a = bones_[i];
        if (!freeBoneFragment(a)) {
            continue;
        }

        for (std::size_t j = i + 1; j < count; ++j) {
            BoneSegment& b = bones_[j];
            if (!freeBoneFragment(b)) {
                continue;
            }

            const SegmentClosestPoints closest = closestSegmentPoints(a.a, a.b, b.a, b.b);
            const double targetDistance = a.radius + b.radius + materials_.fragmentRepulsionSlop;
            if (closest.distance >= targetDistance) {
                continue;
            }

            Vec2 normal = normalized(subtract(closest.pointA, closest.pointB),
                                     normalized(subtract(midpoint(a.a, a.b), midpoint(b.a, b.b)),
                                                normalized({-(a.b.y - a.a.y), a.b.x - a.a.x}, {1.0, 0.0})));
            const double overlap = targetDistance - closest.distance;
            const double massA = std::max(1.0, a.restLength * a.radius * a.radius * (a.splinter ? 0.35 : 1.0));
            const double massB = std::max(1.0, b.restLength * b.radius * b.radius * (b.splinter ? 0.35 : 1.0));
            const double invMassA = 1.0 / massA;
            const double invMassB = 1.0 / massB;
            const double invMassSum = invMassA + invMassB;
            if (invMassSum <= kEpsilon) {
                continue;
            }

            const double correction = overlap * materials_.fragmentRepulsionStiffness;
            const double shareA = invMassA / invMassSum;
            const double shareB = invMassB / invMassSum;
            applyBoneAnchorDelta(a, closest.tA, normal.x * correction * shareA, normal.y * correction * shareA);
            applyBoneAnchorDelta(b, closest.tB, -normal.x * correction * shareB, -normal.y * correction * shareB);

            ++debug_.fragmentPairContacts;
            debug_.maxFragmentOverlap = std::max(debug_.maxFragmentOverlap, overlap);
        }
    }
}

void World::collideBoneFragments() {
    auto processTip = [&](BoneSegment& bone, Vec2 tip, Vec2 previousTip, Vec2 normal, bool strongTip) {
        const double travel = distance(tip, previousTip);
        const double speed = travel / std::max(materials_.fixedDt, kEpsilon);
        const double radius = std::max(materials_.fragmentContactRadius, bone.radius * (strongTip ? 1.75 : 1.25));
        const double impulse = bone.load * 0.22 + speed * bone.radius * (bone.splinter ? 0.75 : 1.0);
        if (impulse < materials_.fragmentDamageImpulse * 0.34) {
            return;
        }
        debug_.maxFragmentImpulse = std::max(debug_.maxFragmentImpulse, impulse);

        const Vec2 tipNormal = normalized(normal, {0.0, -1.0});
        for (Point& point : points_) {
            if (point.pinned) {
                continue;
            }
            const double dx = point.position.x - tip.x;
            const double dy = point.position.y - tip.y;
            const double d = std::sqrt(dx * dx + dy * dy);
            if (d > radius || d <= kEpsilon) {
                continue;
            }

            const bool deepTissue = point.layer == TissueLayer::Muscle || point.exposure > 0.20 || impulse > materials_.fragmentDamageImpulse * 1.35;
            if (!deepTissue) {
                continue;
            }

            const double contact = 1.0 - d / radius;
            debug_.maxFragmentDepth = std::max(debug_.maxFragmentDepth, radius - d);
            const Vec2 away = normalized({dx, dy}, tipNormal);
            const double push = contact * materials_.fragmentPush * (bone.splinter ? 1.25 : 1.0);
            point.position.x += (away.x * radius * 0.42 + tipNormal.x * radius * 0.18) * push;
            point.position.y += (away.y * radius * 0.42 + tipNormal.y * radius * 0.18) * push;
            point.load = std::max(point.load, impulse * contact * 0.46);
            point.exposure = std::max(point.exposure, point.layer == TissueLayer::Muscle ? 1.0 : 0.86);
            ++stats_.fragmentTissueHits;
            ++debug_.fragmentContacts;
            debug_.maxPointLoad = std::max(debug_.maxPointLoad, point.load);
            applyBoneTorque(bone,
                            tip,
                            {-away.x * impulse * contact * 0.06 - tipNormal.x * impulse * contact * 0.02,
                             -away.y * impulse * contact * 0.06 - tipNormal.y * impulse * contact * 0.02});
            if (contact > 0.62 || impulse > materials_.fragmentDamageImpulse) {
                emitFluid(point.position,
                          {away.x + tipNormal.x * 0.45, away.y + tipNormal.y * 0.45 - 0.15},
                          1 + static_cast<int>(std::clamp(impulse / 1200.0, 0.0, 4.0)),
                          80.0 + impulse * materials_.fluidImpactScale * 0.28,
                          point.layer == TissueLayer::Muscle ? 1.7 : 1.9,
                          point.layer == TissueLayer::Muscle ? 0.72 : 0.92);
                openWound(point.position,
                          {away.x + tipNormal.x * 0.45, away.y + tipNormal.y * 0.45 - 0.15},
                          point.layer,
                          impulse / (point.layer == TissueLayer::Muscle ? 1500.0 : 1750.0),
                          point.layer == TissueLayer::Muscle ? 1.7 : 1.9,
                          point.layer == TissueLayer::Muscle ? 0.72 : 0.56);
            }
        }

        for (Spring& spring : springs_) {
            if (spring.broken) {
                continue;
            }
            Point& a = points_[spring.a];
            Point& b = points_[spring.b];
            const Vec2 midpoint{(a.position.x + b.position.x) * 0.5, (a.position.y + b.position.y) * 0.5};
            const double d = distance(midpoint, tip);
            if (d > radius * 1.18) {
                continue;
            }
            const bool reachable = spring.layer == TissueLayer::Muscle ||
                                   std::max(a.exposure, b.exposure) > 0.35 ||
                                   impulse > materials_.fragmentDamageImpulse * 1.75;
            if (!reachable) {
                continue;
            }

            const double contact = 1.0 - d / (radius * 1.18);
            const double threshold = spring.tearImpulse * (spring.layer == TissueLayer::Muscle ? 0.46 : 0.72);
            spring.stress = std::max(spring.stress, impulse * contact / std::max(1.0, threshold));
            if (impulse * contact <= threshold) {
                continue;
            }

            spring.broken = true;
            a.exposure = std::max(a.exposure, 1.0);
            b.exposure = std::max(b.exposure, 1.0);
            a.load = std::max(a.load, impulse * contact * 0.35);
            b.load = std::max(b.load, impulse * contact * 0.35);
            if (spring.layer == TissueLayer::Skin) {
                ++stats_.brokenSkin;
            } else {
                ++stats_.brokenMuscle;
            }
            ++stats_.fragmentTissueTears;
            ++debug_.fragmentTears;
            emitFluid(midpoint,
                      {midpoint.x - tip.x + tipNormal.x * 0.5, midpoint.y - tip.y + tipNormal.y * 0.5 - 0.18},
                      spring.layer == TissueLayer::Skin ? 4 : 3,
                      115.0 + impulse * materials_.fluidImpactScale * 0.36,
                      spring.layer == TissueLayer::Skin ? 2.1 : 1.8,
                      spring.layer == TissueLayer::Skin ? 1.02 : 0.82);
            openWound(midpoint,
                      {midpoint.x - tip.x + tipNormal.x * 0.5, midpoint.y - tip.y + tipNormal.y * 0.5 - 0.18},
                      spring.layer,
                      impulse / (spring.layer == TissueLayer::Skin ? 1450.0 : 1250.0),
                      spring.layer == TissueLayer::Skin ? 2.1 : 1.8,
                      spring.layer == TissueLayer::Skin ? 0.62 : 0.95);
        }

        for (BoneAttachment& attachment : boneAttachments_) {
            if (attachment.broken || attachment.point >= points_.size()) {
                continue;
            }
            Point& point = points_[attachment.point];
            const double d = distance(point.position, tip);
            if (d > radius * 1.12 || impulse < materials_.boneAttachmentBreakImpulse * 0.38) {
                continue;
            }
            attachment.broken = true;
            attachment.stress = std::max(attachment.stress, impulse / std::max(1.0, materials_.boneAttachmentBreakImpulse));
            point.exposure = std::max(point.exposure, 0.95);
            point.load = std::max(point.load, impulse * 0.32);
            ++stats_.brokenBoneAttachments;
            ++stats_.fragmentTissueTears;
            ++debug_.fragmentTears;
            emitFluid(point.position,
                      {point.position.x - tip.x + tipNormal.x * 0.4, point.position.y - tip.y + tipNormal.y * 0.4 - 0.20},
                      3,
                      100.0 + impulse * materials_.fluidImpactScale * 0.30,
                      1.9,
                      0.90);
            openWound(point.position,
                      {point.position.x - tip.x + tipNormal.x * 0.4, point.position.y - tip.y + tipNormal.y * 0.4 - 0.20},
                      TissueLayer::Muscle,
                      impulse / 1350.0,
                      1.9,
                      0.88);
        }

        for (Triangle& triangle : triangles_) {
            if (triangle.failed || triangle.layer != TissueLayer::Muscle) {
                continue;
            }
            Point& a = points_[triangle.a];
            Point& b = points_[triangle.b];
            Point& c = points_[triangle.c];
            const Vec2 centroid{(a.position.x + b.position.x + c.position.x) / 3.0,
                                (a.position.y + b.position.y + c.position.y) / 3.0};
            const double d = distance(centroid, tip);
            if (d > radius * 1.28) {
                continue;
            }
            const double contact = 1.0 - d / (radius * 1.28);
            triangle.damage = std::min(1.35, triangle.damage + contact * impulse / 1800.0);
            if (triangle.damage > 1.0) {
                triangle.failed = true;
                a.exposure = std::max(a.exposure, 1.0);
                b.exposure = std::max(b.exposure, 1.0);
                c.exposure = std::max(c.exposure, 1.0);
                ++stats_.fragmentTissueTears;
                ++debug_.fragmentTears;
            }
        }
    };

    const std::size_t initialBoneCount = bones_.size();
    for (std::size_t i = 0; i < initialBoneCount; ++i) {
        BoneSegment& bone = bones_[i];
        if (bone.pinned || (!bone.fractured && !bone.splinter)) {
            continue;
        }
        if (bone.brokenStart || bone.splinter) {
            processTip(bone, bone.a, bone.previousA, bone.brokenStartNormal, bone.brokenStart);
        }
        if (bone.brokenEnd || bone.splinter) {
            processTip(bone, bone.b, bone.previousB, bone.brokenEndNormal, bone.brokenEnd);
        }
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

    const std::size_t headBone = world.addBoneSegment(bodyPoint(0.0, 0.06), bodyPoint(0.0, 0.17), 10.0, materials.boneFractureImpulse * 0.75, true);
    const std::size_t spineBone = world.addBoneSegment(bodyPoint(0.0, 0.20), bodyPoint(0.0, 0.63), 8.0, materials.boneFractureImpulse);
    const std::size_t shoulderBone = world.addBoneSegment(bodyPoint(-0.14, 0.30), bodyPoint(0.14, 0.30), 7.0, materials.boneFractureImpulse * 0.95);
    const std::size_t pelvisBone = world.addBoneSegment(bodyPoint(-0.10, 0.48), bodyPoint(0.10, 0.48), 6.0, materials.boneFractureImpulse * 0.9);
    const std::size_t leftArmBone = world.addBoneSegment(bodyPoint(-0.195, 0.295), bodyPoint(-0.245, 0.60), 7.0, materials.boneFractureImpulse * 0.82);
    const std::size_t rightArmBone = world.addBoneSegment(bodyPoint(0.195, 0.295), bodyPoint(0.245, 0.60), 7.0, materials.boneFractureImpulse * 0.82);
    const std::size_t leftLegBone = world.addBoneSegment(bodyPoint(-0.065, 0.68), bodyPoint(-0.082, 0.95), 8.0, materials.boneFractureImpulse * 0.9);
    const std::size_t rightLegBone = world.addBoneSegment(bodyPoint(0.065, 0.68), bodyPoint(0.082, 0.95), 8.0, materials.boneFractureImpulse * 0.9);

    world.addBoneJoint(headBone, 1.0, spineBone, 0.0, -0.45, 0.45);
    world.addBoneJoint(spineBone, 0.25, shoulderBone, 0.5, -0.55, 0.55);
    world.addBoneJoint(spineBone, 0.66, pelvisBone, 0.5, -0.45, 0.45);
    world.addBoneJoint(shoulderBone, 0.0, leftArmBone, 0.0, -1.15, 1.15);
    world.addBoneJoint(shoulderBone, 1.0, rightArmBone, 0.0, -1.15, 1.15);
    world.addBoneJoint(pelvisBone, 0.18, leftLegBone, 0.0, -0.75, 0.75);
    world.addBoneJoint(pelvisBone, 0.82, rightLegBone, 0.0, -0.75, 0.75);

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
