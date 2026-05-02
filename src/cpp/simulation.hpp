#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

namespace rp {

struct Vec2 {
    double x = 0.0;
    double y = 0.0;
};

enum class TissueLayer {
    Skin,
    Muscle
};

struct InputState {
    bool active = false;
    bool down = false;
    double x = 0.0;
    double y = 0.0;
    double vx = 0.0;
    double vy = 0.0;
    double power = 2.0;
};

struct Materials {
    double fixedDt = 1.0 / 60.0;
    int solverIterations = 9;
    double gravity = 920.0;
    double damping = 0.992;
    double pointSpacing = 18.0;
    double floorFriction = 0.78;
    double strikerRadius = 34.0;
    double strikerMass = 2.9;
    double directMuscleContact = 0.18;
    double skinShapeStiffness = 0.006;
    double muscleShapeStiffness = 0.018;

    double skinStructuralStiffness = 0.82;
    double skinShearStiffness = 0.48;
    double skinAreaStiffness = 0.03;
    double skinTearStretch = 1.68;
    double skinTearImpulse = 820.0;

    double muscleFiberStiffness = 0.74;
    double muscleCrossStiffness = 0.34;
    double muscleShearStiffness = 0.28;
    double muscleAreaStiffness = 0.24;
    double muscleTearStretch = 1.92;
    double muscleTearImpulse = 1180.0;
    double muscleExposedTearImpulse = 620.0;

    double attachmentStiffness = 0.19;
    double attachmentBreakStretch = 2.25;
    double attachmentBreakImpulse = 760.0;
};

struct Point {
    Vec2 position;
    Vec2 previous;
    Vec2 home;
    TissueLayer layer = TissueLayer::Skin;
    bool pinned = false;
    double load = 0.0;
    double exposure = 0.0;
    double mass = 1.0;
};

struct Spring {
    std::size_t a = 0;
    std::size_t b = 0;
    double rest = 0.0;
    double stiffness = 0.0;
    double tearStretch = 0.0;
    double tearImpulse = 0.0;
    TissueLayer layer = TissueLayer::Skin;
    bool fiber = false;
    bool broken = false;
    double stress = 0.0;
};

struct AreaConstraint {
    std::size_t a = 0;
    std::size_t b = 0;
    std::size_t c = 0;
    double restArea = 0.0;
    double stiffness = 0.0;
    TissueLayer layer = TissueLayer::Skin;
};

struct Attachment {
    std::size_t skinPoint = 0;
    std::size_t musclePoint = 0;
    double rest = 0.0;
    bool broken = false;
    double stress = 0.0;
};

struct Triangle {
    std::size_t a = 0;
    std::size_t b = 0;
    std::size_t c = 0;
    TissueLayer layer = TissueLayer::Skin;
    bool failed = false;
    double damage = 0.0;
};

struct Stats {
    int brokenSkin = 0;
    int brokenMuscle = 0;
    int brokenAttachments = 0;
};

class World {
public:
    explicit World(Materials materials = {});

    std::size_t addPoint(Vec2 position, TissueLayer layer, bool pinned);
    void addSpring(std::size_t a, std::size_t b, TissueLayer layer, double stiffness, double tearStretch, double tearImpulse, bool fiber = false);
    void addArea(std::size_t a, std::size_t b, std::size_t c, TissueLayer layer, double stiffness);
    void addAttachment(std::size_t skinPoint, std::size_t musclePoint);
    void addTriangle(std::size_t a, std::size_t b, std::size_t c, TissueLayer layer);

    void step(double dt, const InputState& input, double width, double height);
    bool triangleAlive(const Triangle& triangle) const;
    bool hasLiveSpring(std::size_t a, std::size_t b, TissueLayer layer) const;

    const Materials& materials() const { return materials_; }
    const std::vector<Point>& points() const { return points_; }
    const std::vector<Spring>& springs() const { return springs_; }
    const std::vector<AreaConstraint>& areas() const { return areas_; }
    const std::vector<Attachment>& attachments() const { return attachments_; }
    const std::vector<Triangle>& triangles() const { return triangles_; }
    const Stats& stats() const { return stats_; }

private:
    void integrate(double dt, double width, double floorY);
    void collideStriker(double dt, const InputState& input);
    void solveSprings();
    void solveAttachments();
    void solveAreas();
    void constrainToWorld(double width, double floorY);
    void updateExposure();
    void updateTriangleDamage();

    Materials materials_;
    std::vector<Point> points_;
    std::vector<Spring> springs_;
    std::vector<AreaConstraint> areas_;
    std::vector<Attachment> attachments_;
    std::vector<Triangle> triangles_;
    Stats stats_;
};

World createLayeredBody(double width, double height, Materials materials = {});

double distance(Vec2 a, Vec2 b);
double signedArea(Vec2 a, Vec2 b, Vec2 c);

} // namespace rp
