#pragma once

#include <cstddef>
#include <cstdint>
#include <limits>
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

inline constexpr std::size_t kMissingSpring = std::numeric_limits<std::size_t>::max();

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

    double boneFractureImpulse = 1850.0;
    int maxBoneFractureDepth = 3;
    double minBoneFragmentLength = 30.0;
    double boneDamping = 0.988;
    double boneShapeStiffness = 0.004;
    double boneAttachmentStiffness = 0.38;
    double boneAttachmentBreakImpulse = 2100.0;
    double boneAttachmentBreakStretch = 2.8;
    double boneJointStiffness = 0.54;
    double boneJointAngularStiffness = 0.16;
    double boneJointBreakStretch = 2.15;
    double boneJointBreakImpulse = 2600.0;
    double boneJointAngularBreak = 1.20;
    double boneImpactTransfer = 0.62;
    double boneDirectContact = 0.86;
    double boneDirectPressure = 780.0;
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
    std::size_t edgeAB = kMissingSpring;
    std::size_t edgeBC = kMissingSpring;
    std::size_t edgeCA = kMissingSpring;
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
    std::size_t edgeAB = kMissingSpring;
    std::size_t edgeBC = kMissingSpring;
    std::size_t edgeCA = kMissingSpring;
    TissueLayer layer = TissueLayer::Skin;
    bool failed = false;
    double damage = 0.0;
};

struct BoneSegment {
    Vec2 a;
    Vec2 b;
    Vec2 previousA;
    Vec2 previousB;
    Vec2 homeA;
    Vec2 homeB;
    double radius = 5.0;
    double restLength = 1.0;
    double fractureImpulse = 2600.0;
    double load = 0.0;
    bool fractured = false;
    bool brokenStart = false;
    bool brokenEnd = false;
    Vec2 brokenStartNormal;
    Vec2 brokenEndNormal;
    int fractureGeneration = 0;
    bool splinter = false;
    bool pinned = false;
};

struct BoneAttachment {
    std::size_t point = 0;
    std::size_t bone = 0;
    double t = 0.0;
    Vec2 offset;
    double rest = 0.0;
    double stress = 0.0;
    bool broken = false;
};

struct BoneJoint {
    std::size_t a = 0;
    std::size_t b = 0;
    double tA = 0.0;
    double tB = 0.0;
    double rest = 0.0;
    double restAngle = 0.0;
    double minAngle = -0.95;
    double maxAngle = 0.95;
    double stress = 0.0;
    double torqueStress = 0.0;
    bool broken = false;
};

struct Stats {
    int brokenSkin = 0;
    int brokenMuscle = 0;
    int brokenAttachments = 0;
    int brokenBoneAttachments = 0;
    int brokenBoneJoints = 0;
    int fracturedBones = 0;
};

struct ContactDebug {
    bool active = false;
    bool down = false;
    Vec2 strikerPosition;
    Vec2 strikerVelocity;
    Vec2 strongestContact;
    double strikerSpeed = 0.0;
    double strikerMass = 0.0;
    double strikerRadius = 0.0;
    double impact = 0.0;
    double maxDepth = 0.0;
    double maxBoneLoad = 0.0;
    double maxPointLoad = 0.0;
    double lastFractureImpulse = 0.0;
    int boneContacts = 0;
    int tissueContacts = 0;
    int fractures = 0;
};

struct AnatomyValidation {
    int skinPoints = 0;
    int musclePoints = 0;
    int boneSamples = 0;
    int boneSamplesOutsideSkin = 0;
    int boneSamplesOutsideMuscle = 0;
    int boneSegmentsOutsideSkin = 0;
    int boneSegmentsOutsideMuscle = 0;
};

class World {
public:
    explicit World(Materials materials = {});

    std::size_t addPoint(Vec2 position, TissueLayer layer, bool pinned);
    void addSpring(std::size_t a, std::size_t b, TissueLayer layer, double stiffness, double tearStretch, double tearImpulse, bool fiber = false);
    void addArea(std::size_t a, std::size_t b, std::size_t c, TissueLayer layer, double stiffness);
    void addAttachment(std::size_t skinPoint, std::size_t musclePoint);
    void addTriangle(std::size_t a, std::size_t b, std::size_t c, TissueLayer layer);
    std::size_t addBoneSegment(Vec2 a, Vec2 b, double radius, double fractureImpulse, bool pinned = false);
    void addBoneAttachment(std::size_t point, std::size_t bone, double t);
    void addBoneJoint(std::size_t a, double tA, std::size_t b, double tB, double minAngle = -0.95, double maxAngle = 0.95);

    void step(double dt, const InputState& input, double width, double height);
    bool triangleAlive(const Triangle& triangle) const;
    bool hasLiveSpring(std::size_t a, std::size_t b, TissueLayer layer) const;

    const Materials& materials() const { return materials_; }
    const std::vector<Point>& points() const { return points_; }
    const std::vector<Spring>& springs() const { return springs_; }
    const std::vector<AreaConstraint>& areas() const { return areas_; }
    const std::vector<Attachment>& attachments() const { return attachments_; }
    const std::vector<Triangle>& triangles() const { return triangles_; }
    const std::vector<BoneSegment>& bones() const { return bones_; }
    const std::vector<BoneAttachment>& boneAttachments() const { return boneAttachments_; }
    const std::vector<BoneJoint>& boneJoints() const { return boneJoints_; }
    const Stats& stats() const { return stats_; }
    const ContactDebug& debug() const { return debug_; }

private:
    std::size_t findSpringIndex(std::size_t a, std::size_t b, TissueLayer layer) const;
    bool springAlive(std::size_t springIndex) const;
    int liveEdgeCount(std::size_t edgeAB, std::size_t edgeBC, std::size_t edgeCA) const;
    Vec2 bonePoint(const BoneSegment& bone, double t) const;
    bool canFractureBone(const BoneSegment& bone) const;
    void fractureBone(std::size_t boneIndex, double fractureT = 0.5, Vec2 impulseNormal = {}, double impulse = 0.0);
    void damageTissueAroundFracture(Vec2 center, double radius, double impulse);
    void applyBoneAnchorDelta(BoneSegment& bone, double t, double dx, double dy);
    void rotateBoneAroundAnchor(BoneSegment& bone, double t, double angle);

    void integrate(double dt, double width, double floorY);
    void collideStriker(double dt, const InputState& input);
    void solveSprings();
    void solveAttachments();
    void solveBoneAttachments();
    void solveBoneJoints();
    void solveBones();
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
    std::vector<BoneSegment> bones_;
    std::vector<BoneAttachment> boneAttachments_;
    std::vector<BoneJoint> boneJoints_;
    Stats stats_;
    ContactDebug debug_;
};

World createLayeredBody(double width, double height, Materials materials = {});

double distance(Vec2 a, Vec2 b);
double signedArea(Vec2 a, Vec2 b, Vec2 c);
bool pointInsideLayer(const World& world, Vec2 point, TissueLayer layer);
AnatomyValidation validateAnatomy(const World& world, int samplesPerBone = 16);

} // namespace rp
