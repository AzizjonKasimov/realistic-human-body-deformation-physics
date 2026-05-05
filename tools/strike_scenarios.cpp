#include "simulation.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <limits>
#include <string>
#include <vector>

namespace {

struct IntBand {
    int min = 0;
    int max = std::numeric_limits<int>::max();
};

struct DoubleBand {
    double min = 0.0;
    double max = std::numeric_limits<double>::infinity();
};

struct ScenarioExpectations {
    IntBand contacts{1, std::numeric_limits<int>::max()};
    IntBand boneFractures{0, std::numeric_limits<int>::max()};
    IntBand skinTears{0, std::numeric_limits<int>::max()};
    IntBand fluidEmitted{0, std::numeric_limits<int>::max()};
    IntBand woundFluid{0, std::numeric_limits<int>::max()};
    IntBand openedWounds{0, std::numeric_limits<int>::max()};
    IntBand fragmentPairContacts{0, std::numeric_limits<int>::max()};
    IntBand jointCorrections{0, std::numeric_limits<int>::max()};
    DoubleBand fragmentOverlap{0.0, 18.0};
    DoubleBand boneSpin{0.0, 28.0};
};

struct Scenario {
    const char* name = "";
    const char* region = "";
    const char* intent = "";
    rp::ToolMode tool = rp::ToolMode::Blunt;
    rp::Vec2 start;
    rp::Vec2 end;
    int windupFrames = 12;
    int strikeFrames = 34;
    int settleFrames = 42;
    double power = 4.0;
    ScenarioExpectations expectations;
};

const char* toolName(rp::ToolMode tool) {
    switch (tool) {
    case rp::ToolMode::Sharp:
        return "sharp";
    case rp::ToolMode::Heavy:
        return "heavy";
    case rp::ToolMode::Blunt:
    default:
        return "blunt";
    }
}

struct ScenarioResult {
    int tissueContacts = 0;
    int boneContacts = 0;
    int fractures = 0;
    int skinTears = 0;
    int muscleTears = 0;
    int detachments = 0;
    int boneDetachments = 0;
    int boneJointBreaks = 0;
    int boneFractures = 0;
    int finalBones = 0;
    int fluidEmitted = 0;
    int woundFluid = 0;
    int openedWounds = 0;
    int maxActiveWounds = 0;
    int woundLeaks = 0;
    int maxActiveFluids = 0;
    int fragmentHits = 0;
    int fragmentTears = 0;
    int fragmentPairContacts = 0;
    int postFractureJointCorrections = 0;
    double maxImpact = 0.0;
    double maxBoneLoad = 0.0;
    double maxPointLoad = 0.0;
    double maxDepth = 0.0;
    double maxFragmentDepth = 0.0;
    double maxFragmentImpulse = 0.0;
    double maxFragmentOverlap = 0.0;
    double maxPostFractureJointStretch = 0.0;
    double maxPostFractureJointAngle = 0.0;
    double maxWoundPressure = 0.0;
    double maxWoundClot = 0.0;
    double maxBoneAngularSpeed = 0.0;
    int finalFreeFragments = 0;
    int finalSpinningFragments = 0;
};

int activeFluidCount(const rp::World& world) {
    return static_cast<int>(std::count_if(world.fluids().begin(), world.fluids().end(), [](const rp::FluidParticle& fluid) {
        return fluid.life > 0.0;
    }));
}

bool freeFragment(const rp::BoneSegment& bone) {
    return !bone.pinned && (bone.fractured || bone.splinter);
}

int freeFragmentCount(const rp::World& world) {
    return static_cast<int>(std::count_if(world.bones().begin(), world.bones().end(), [](const rp::BoneSegment& bone) {
        return freeFragment(bone);
    }));
}

int spinningFragmentCount(const rp::World& world) {
    return static_cast<int>(std::count_if(world.bones().begin(), world.bones().end(), [](const rp::BoneSegment& bone) {
        return freeFragment(bone) && std::abs(bone.angularVelocity) > 0.08;
    }));
}

rp::InputState makeStrikeInput(const Scenario& scenario, int frame, double dt) {
    const double t0 = static_cast<double>(std::max(0, frame - scenario.windupFrames)) /
                      static_cast<double>(std::max(1, scenario.strikeFrames - 1));
    const double t = std::clamp(t0, 0.0, 1.0);
    const rp::Vec2 position{
        scenario.start.x + (scenario.end.x - scenario.start.x) * t,
        scenario.start.y + (scenario.end.y - scenario.start.y) * t,
    };
    const rp::Vec2 velocity{
        (scenario.end.x - scenario.start.x) / (static_cast<double>(std::max(1, scenario.strikeFrames - 1)) * dt),
        (scenario.end.y - scenario.start.y) / (static_cast<double>(std::max(1, scenario.strikeFrames - 1)) * dt),
    };

    rp::InputState input;
    input.down = frame >= scenario.windupFrames && frame < scenario.windupFrames + scenario.strikeFrames;
    input.active = input.down;
    input.x = position.x;
    input.y = position.y;
    input.vx = input.down ? velocity.x : 0.0;
    input.vy = input.down ? velocity.y : 0.0;
    input.power = scenario.power;
    input.tool = scenario.tool;
    return input;
}

void writeFrame(std::ostream& out, const Scenario& scenario, int frame, const rp::World& world) {
    const rp::ContactDebug& debug = world.debug();
    out << scenario.name << ','
        << scenario.region << ','
        << scenario.intent << ','
        << toolName(debug.tool) << ','
        << frame << ','
        << debug.strikerPosition.x << ','
        << debug.strikerPosition.y << ','
        << debug.strikerSpeed << ','
        << debug.impact << ','
        << debug.tissueContacts << ','
        << debug.boneContacts << ','
        << debug.maxDepth << ','
        << debug.maxPointLoad << ','
        << debug.maxBoneLoad << ','
        << debug.fractures << ','
        << world.stats().brokenSkin << ','
        << world.stats().brokenMuscle << ','
        << world.stats().brokenAttachments << ','
        << world.stats().brokenBoneAttachments << ','
        << world.stats().brokenBoneJoints << ','
        << world.stats().fracturedBones << ','
        << debug.fluidEmitted << ','
        << activeFluidCount(world) << ','
        << world.stats().emittedFluidParticles << ','
        << world.stats().openedWounds << ','
        << debug.activeWounds << ','
        << debug.woundLeaks << ','
        << world.stats().woundFluidParticles << ','
        << debug.maxWoundPressure << ','
        << debug.maxWoundClot << ','
        << debug.fragmentContacts << ','
        << debug.fragmentTears << ','
        << debug.fragmentPairContacts << ','
        << debug.maxFragmentDepth << ','
        << debug.maxFragmentImpulse << ','
        << debug.maxFragmentOverlap << ','
        << debug.postFractureJointCorrections << ','
        << debug.maxPostFractureJointStretch << ','
        << debug.maxPostFractureJointAngle << ','
        << world.stats().fragmentTissueHits << ','
        << world.stats().fragmentTissueTears << ','
        << debug.maxBoneAngularSpeed << ','
        << freeFragmentCount(world) << ','
        << spinningFragmentCount(world) << '\n';
}

ScenarioResult runScenario(const Scenario& scenario, std::ostream& csv) {
    constexpr double width = 1280.0;
    constexpr double height = 720.0;
    rp::World world = rp::createLayeredBody(width, height);
    ScenarioResult result;
    const double dt = world.materials().fixedDt;
    const int totalFrames = scenario.windupFrames + scenario.strikeFrames + scenario.settleFrames;

    for (int frame = 0; frame < totalFrames; ++frame) {
        rp::InputState input;
        if (frame < scenario.windupFrames + scenario.strikeFrames) {
            input = makeStrikeInput(scenario, frame, dt);
        }
        world.step(dt, input, width, height);
        const rp::ContactDebug& debug = world.debug();
        result.tissueContacts += debug.tissueContacts;
        result.boneContacts += debug.boneContacts;
        result.fractures += debug.fractures;
        result.maxImpact = std::max(result.maxImpact, debug.impact);
        result.maxBoneLoad = std::max(result.maxBoneLoad, debug.maxBoneLoad);
        result.maxPointLoad = std::max(result.maxPointLoad, debug.maxPointLoad);
        result.maxDepth = std::max(result.maxDepth, debug.maxDepth);
        result.maxFragmentDepth = std::max(result.maxFragmentDepth, debug.maxFragmentDepth);
        result.maxFragmentImpulse = std::max(result.maxFragmentImpulse, debug.maxFragmentImpulse);
        result.maxFragmentOverlap = std::max(result.maxFragmentOverlap, debug.maxFragmentOverlap);
        result.maxPostFractureJointStretch = std::max(result.maxPostFractureJointStretch, debug.maxPostFractureJointStretch);
        result.maxPostFractureJointAngle = std::max(result.maxPostFractureJointAngle, debug.maxPostFractureJointAngle);
        result.maxWoundPressure = std::max(result.maxWoundPressure, debug.maxWoundPressure);
        result.maxWoundClot = std::max(result.maxWoundClot, debug.maxWoundClot);
        result.maxBoneAngularSpeed = std::max(result.maxBoneAngularSpeed, debug.maxBoneAngularSpeed);
        result.fragmentPairContacts += debug.fragmentPairContacts;
        result.postFractureJointCorrections += debug.postFractureJointCorrections;
        result.maxActiveWounds = std::max(result.maxActiveWounds, debug.activeWounds);
        result.woundLeaks += debug.woundLeaks;
        result.maxActiveFluids = std::max(result.maxActiveFluids, activeFluidCount(world));
        writeFrame(csv, scenario, frame, world);
    }

    result.skinTears = world.stats().brokenSkin;
    result.muscleTears = world.stats().brokenMuscle;
    result.detachments = world.stats().brokenAttachments;
    result.boneDetachments = world.stats().brokenBoneAttachments;
    result.boneJointBreaks = world.stats().brokenBoneJoints;
    result.boneFractures = world.stats().fracturedBones;
    result.finalBones = static_cast<int>(world.bones().size());
    result.fluidEmitted = world.stats().emittedFluidParticles;
    result.woundFluid = world.stats().woundFluidParticles;
    result.openedWounds = world.stats().openedWounds;
    result.fragmentHits = world.stats().fragmentTissueHits;
    result.fragmentTears = world.stats().fragmentTissueTears;
    result.finalFreeFragments = freeFragmentCount(world);
    result.finalSpinningFragments = spinningFragmentCount(world);
    return result;
}

void writeSummary(std::ostream& out, const Scenario& scenario, const ScenarioResult& result) {
    out << scenario.name << ','
        << scenario.region << ','
        << scenario.intent << ','
        << toolName(scenario.tool) << ','
        << scenario.power << ','
        << scenario.strikeFrames << ','
        << result.tissueContacts << ','
        << result.boneContacts << ','
        << result.fractures << ','
        << result.maxImpact << ','
        << result.maxPointLoad << ','
        << result.maxBoneLoad << ','
        << result.maxDepth << ','
        << result.skinTears << ','
        << result.muscleTears << ','
        << result.detachments << ','
        << result.boneDetachments << ','
        << result.boneJointBreaks << ','
        << result.boneFractures << ','
        << result.finalBones << ','
        << result.fluidEmitted << ','
        << result.openedWounds << ','
        << result.woundFluid << ','
        << result.maxActiveWounds << ','
        << result.woundLeaks << ','
        << result.maxWoundPressure << ','
        << result.maxWoundClot << ','
        << result.maxActiveFluids << ','
        << result.fragmentHits << ','
        << result.fragmentTears << ','
        << result.fragmentPairContacts << ','
        << result.maxFragmentDepth << ','
        << result.maxFragmentImpulse << ','
        << result.maxFragmentOverlap << ','
        << result.postFractureJointCorrections << ','
        << result.maxPostFractureJointStretch << ','
        << result.maxPostFractureJointAngle << ','
        << result.maxBoneAngularSpeed << ','
        << result.finalFreeFragments << ','
        << result.finalSpinningFragments << '\n';
}

Scenario makeScenario(const char* name,
                      const char* region,
                      const char* intent,
                      rp::ToolMode tool,
                      rp::Vec2 start,
                      rp::Vec2 end,
                      int strikeFrames,
                      double power,
                      ScenarioExpectations expectations,
                      int settleFrames = 54,
                      int windupFrames = 12) {
    Scenario scenario;
    scenario.name = name;
    scenario.region = region;
    scenario.intent = intent;
    scenario.tool = tool;
    scenario.start = start;
    scenario.end = end;
    scenario.windupFrames = windupFrames;
    scenario.strikeFrames = strikeFrames;
    scenario.settleFrames = settleFrames;
    scenario.power = power;
    scenario.expectations = expectations;
    return scenario;
}

ScenarioExpectations lowDamageExpectations() {
    ScenarioExpectations expectations;
    expectations.boneFractures = {0, 2};
    expectations.skinTears = {0, 260};
    expectations.fluidEmitted = {0, 3400};
    expectations.woundFluid = {0, 900};
    expectations.openedWounds = {0, 80};
    expectations.fragmentPairContacts = {0, 180};
    expectations.jointCorrections = {0, 400};
    expectations.fragmentOverlap = {0.0, 12.0};
    expectations.boneSpin = {0.0, 8.0};
    return expectations;
}

ScenarioExpectations limbBendExpectations() {
    ScenarioExpectations expectations;
    expectations.contacts = {10, std::numeric_limits<int>::max()};
    expectations.boneFractures = {0, 2};
    expectations.skinTears = {0, 260};
    expectations.fluidEmitted = {0, 3200};
    expectations.woundFluid = {0, 900};
    expectations.openedWounds = {0, 80};
    expectations.fragmentPairContacts = {0, 260};
    expectations.jointCorrections = {0, 400};
    expectations.fragmentOverlap = {0.0, 12.0};
    expectations.boneSpin = {0.0, 10.0};
    return expectations;
}

ScenarioExpectations cutExpectations() {
    ScenarioExpectations expectations;
    expectations.skinTears = {15, 1200};
    expectations.fluidEmitted = {100, 12000};
    expectations.woundFluid = {8, 3800};
    expectations.openedWounds = {1, 150};
    expectations.boneFractures = {0, 8};
    expectations.fragmentPairContacts = {0, 1200};
    expectations.jointCorrections = {0, 1800};
    expectations.fragmentOverlap = {0.0, 16.0};
    expectations.boneSpin = {0.0, 18.0};
    return expectations;
}

ScenarioExpectations bluntExpectations() {
    ScenarioExpectations expectations;
    expectations.skinTears = {10, 1400};
    expectations.fluidEmitted = {200, 20000};
    expectations.woundFluid = {0, 7000};
    expectations.openedWounds = {0, 160};
    expectations.boneFractures = {0, 12};
    expectations.fragmentPairContacts = {0, 2200};
    expectations.jointCorrections = {0, 2600};
    expectations.fragmentOverlap = {0.0, 18.0};
    expectations.boneSpin = {0.0, 22.0};
    return expectations;
}

ScenarioExpectations heavyExpectations() {
    ScenarioExpectations expectations;
    expectations.skinTears = {20, 1600};
    expectations.fluidEmitted = {500, 28000};
    expectations.woundFluid = {30, 9500};
    expectations.openedWounds = {1, 160};
    expectations.boneFractures = {1, 18};
    expectations.fragmentPairContacts = {0, 5200};
    expectations.jointCorrections = {0, 4200};
    expectations.fragmentOverlap = {0.0, 22.0};
    expectations.boneSpin = {0.0, 30.0};
    return expectations;
}

std::vector<Scenario> buildScenarios() {
    std::vector<Scenario> scenarios;
    scenarios.reserve(22);

    scenarios.push_back(makeScenario("torso_probe_blunt", "torso", "low_energy_probe", rp::ToolMode::Blunt, {562.0, 336.0}, {664.0, 336.0}, 42, 1.4, lowDamageExpectations(), 44));
    scenarios.push_back(makeScenario("torso_blunt_medium", "torso", "medium_crush", rp::ToolMode::Blunt, {545.0, 336.0}, {704.0, 336.0}, 34, 4.0, bluntExpectations(), 56));
    scenarios.push_back(makeScenario("torso_blunt_fast", "torso", "fast_crush", rp::ToolMode::Blunt, {520.0, 332.0}, {728.0, 340.0}, 24, 4.0, bluntExpectations(), 60));
    scenarios.push_back(makeScenario("torso_sharp_slash", "torso", "skin_cut", rp::ToolMode::Sharp, {545.0, 336.0}, {704.0, 336.0}, 34, 4.0, cutExpectations(), 54));
    scenarios.push_back(makeScenario("torso_sharp_diagonal", "torso", "diagonal_cut", rp::ToolMode::Sharp, {548.0, 300.0}, {710.0, 388.0}, 30, 3.6, cutExpectations(), 54));
    scenarios.push_back(makeScenario("torso_heavy_drive", "torso", "deep_fracture", rp::ToolMode::Heavy, {545.0, 336.0}, {704.0, 336.0}, 34, 4.0, heavyExpectations(), 64));

    scenarios.push_back(makeScenario("left_shoulder_probe", "left_shoulder", "low_energy_probe", rp::ToolMode::Blunt, {470.0, 238.0}, {570.0, 254.0}, 40, 1.8, lowDamageExpectations(), 44));
    scenarios.push_back(makeScenario("left_shoulder_blunt", "left_shoulder", "joint_transfer", rp::ToolMode::Blunt, {460.0, 238.0}, {592.0, 258.0}, 30, 3.0, bluntExpectations(), 52));
    scenarios.push_back(makeScenario("left_shoulder_sharp", "left_shoulder", "joint_cut", rp::ToolMode::Sharp, {460.0, 238.0}, {592.0, 258.0}, 30, 3.0, cutExpectations(), 52));
    scenarios.push_back(makeScenario("right_shoulder_blunt", "right_shoulder", "joint_transfer", rp::ToolMode::Blunt, {872.0, 238.0}, {740.0, 258.0}, 30, 3.0, bluntExpectations(), 52));
    scenarios.push_back(makeScenario("right_shoulder_heavy", "right_shoulder", "joint_fracture", rp::ToolMode::Heavy, {884.0, 238.0}, {736.0, 264.0}, 28, 3.8, heavyExpectations(), 62));

    scenarios.push_back(makeScenario("left_arm_blunt_sweep", "left_arm", "limb_bend", rp::ToolMode::Blunt, {500.0, 274.0}, {462.0, 448.0}, 34, 3.2, limbBendExpectations(), 58));
    scenarios.push_back(makeScenario("left_arm_sharp_drag", "left_arm", "limb_cut", rp::ToolMode::Sharp, {592.0, 282.0}, {528.0, 446.0}, 32, 3.4, cutExpectations(), 56));
    scenarios.push_back(makeScenario("right_arm_heavy_sweep", "right_arm", "limb_fracture", rp::ToolMode::Heavy, {830.0, 274.0}, {872.0, 452.0}, 30, 3.8, heavyExpectations(), 64));

    scenarios.push_back(makeScenario("hip_blunt_drive", "hip", "pelvis_transfer", rp::ToolMode::Blunt, {548.0, 420.0}, {692.0, 430.0}, 32, 3.4, bluntExpectations(), 58));
    scenarios.push_back(makeScenario("hip_sharp_low", "hip", "low_cut", rp::ToolMode::Sharp, {548.0, 420.0}, {692.0, 430.0}, 32, 3.1, cutExpectations(), 54));
    scenarios.push_back(makeScenario("hip_heavy_drive", "hip", "pelvis_fracture", rp::ToolMode::Heavy, {548.0, 420.0}, {692.0, 430.0}, 32, 4.0, heavyExpectations(), 64));
    scenarios.push_back(makeScenario("hip_heavy_upward", "hip", "upward_fracture", rp::ToolMode::Heavy, {604.0, 502.0}, {690.0, 408.0}, 28, 4.0, heavyExpectations(), 64));

    scenarios.push_back(makeScenario("left_leg_blunt_sweep", "left_leg", "limb_bend", rp::ToolMode::Blunt, {586.0, 470.0}, {540.0, 630.0}, 34, 3.2, limbBendExpectations(), 58));
    scenarios.push_back(makeScenario("left_leg_heavy_drive", "left_leg", "limb_fracture", rp::ToolMode::Heavy, {590.0, 492.0}, {536.0, 642.0}, 30, 4.0, heavyExpectations(), 66));
    scenarios.push_back(makeScenario("right_leg_sharp_sweep", "right_leg", "limb_cut", rp::ToolMode::Sharp, {724.0, 458.0}, {708.0, 636.0}, 34, 3.4, cutExpectations(), 58));
    scenarios.push_back(makeScenario("right_leg_heavy_drive", "right_leg", "limb_fracture", rp::ToolMode::Heavy, {738.0, 492.0}, {792.0, 642.0}, 30, 4.0, heavyExpectations(), 66));

    return scenarios;
}

bool outside(IntBand band, int value) {
    return value < band.min || value > band.max;
}

bool outside(DoubleBand band, double value) {
    return value < band.min || value > band.max;
}

int writeTuningReport(std::ostream& out, const std::vector<Scenario>& scenarios, const std::vector<ScenarioResult>& results) {
    int warnings = 0;
    out << "Strike tuning report\n";
    out << "scenarios=" << scenarios.size() << '\n';
    out << '\n';

    auto writeIntCheck = [&](const char* label, int value, IntBand band) {
        if (!outside(band, value)) {
            return;
        }
        ++warnings;
        out << "  WARN " << label << "=" << value << " expected [" << band.min << ", " << band.max << "]\n";
    };

    auto writeDoubleCheck = [&](const char* label, double value, DoubleBand band) {
        if (!outside(band, value)) {
            return;
        }
        ++warnings;
        out << "  WARN " << label << "=" << value << " expected [" << band.min << ", " << band.max << "]\n";
    };

    for (std::size_t i = 0; i < scenarios.size(); ++i) {
        const Scenario& scenario = scenarios[i];
        const ScenarioResult& result = results[i];
        const ScenarioExpectations& expected = scenario.expectations;
        const int contacts = result.tissueContacts + result.boneContacts;
        const int scenarioWarningsBefore = warnings;
        out << scenario.name
            << " region=" << scenario.region
            << " intent=" << scenario.intent
            << " tool=" << toolName(scenario.tool)
            << " impact=" << result.maxImpact
            << " contacts=" << contacts
            << " fractures=" << result.boneFractures
            << " fluid=" << result.fluidEmitted
            << " wound_fluid=" << result.woundFluid
            << " wounds=" << result.openedWounds
            << " joint_limits=" << result.postFractureJointCorrections
            << '\n';
        writeIntCheck("contacts", contacts, expected.contacts);
        writeIntCheck("bone_fractures", result.boneFractures, expected.boneFractures);
        writeIntCheck("skin_tears", result.skinTears, expected.skinTears);
        writeIntCheck("fluid_emitted", result.fluidEmitted, expected.fluidEmitted);
        writeIntCheck("wound_fluid", result.woundFluid, expected.woundFluid);
        writeIntCheck("opened_wounds", result.openedWounds, expected.openedWounds);
        writeIntCheck("fragment_pair_contacts", result.fragmentPairContacts, expected.fragmentPairContacts);
        writeIntCheck("post_fracture_joint_corrections", result.postFractureJointCorrections, expected.jointCorrections);
        writeDoubleCheck("max_fragment_overlap", result.maxFragmentOverlap, expected.fragmentOverlap);
        writeDoubleCheck("max_bone_angular_speed", result.maxBoneAngularSpeed, expected.boneSpin);
        if (scenarioWarningsBefore == warnings) {
            out << "  OK\n";
        }
    }

    out << '\n';
    out << "warnings=" << warnings << '\n';
    return warnings;
}

} // namespace

int main(int argc, char** argv) {
    const std::filesystem::path output = argc > 1
        ? std::filesystem::path(argv[1])
        : std::filesystem::path("output/strike_scenarios.csv");
    std::filesystem::path summaryOutput = output;
    summaryOutput.replace_filename("strike_summary.csv");
    std::filesystem::path reportOutput = output;
    reportOutput.replace_filename("strike_tuning_report.txt");
    if (output.has_parent_path()) {
        std::filesystem::create_directories(output.parent_path());
    }

    std::ofstream csv(output);
    if (!csv) {
        std::cerr << "failed to write " << output.string() << '\n';
        return 1;
    }
    std::ofstream summaryCsv(summaryOutput);
    if (!summaryCsv) {
        std::cerr << "failed to write " << summaryOutput.string() << '\n';
        return 1;
    }
    std::ofstream report(reportOutput);
    if (!report) {
        std::cerr << "failed to write " << reportOutput.string() << '\n';
        return 1;
    }

    csv << std::fixed << std::setprecision(3);
    summaryCsv << std::fixed << std::setprecision(3);
    report << std::fixed << std::setprecision(3);
    csv << "scenario,region,intent,tool,frame,x,y,speed,impact,tissue_contacts,bone_contacts,max_depth,max_point_load,max_bone_load,step_fractures,skin_tears,muscle_tears,detachments,bone_detachments,bone_joint_breaks,bone_fractures,step_fluid_emitted,active_fluids,total_fluid_emitted,opened_wounds,active_wounds,step_wound_leaks,total_wound_fluid,max_wound_pressure,max_wound_clot,step_fragment_contacts,step_fragment_tears,step_fragment_pair_contacts,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,step_post_fracture_joint_corrections,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,total_fragment_hits,total_fragment_tears,max_bone_angular_speed,free_fragments,spinning_fragments\n";
    summaryCsv << "scenario,region,intent,tool,power,strike_frames,total_tissue_contacts,total_bone_contacts,total_step_fractures,max_impact,max_point_load,max_bone_load,max_depth,skin_tears,muscle_tears,detachments,bone_detachments,bone_joint_breaks,bone_fractures,final_bones,total_fluid_emitted,opened_wounds,total_wound_fluid,max_active_wounds,total_wound_leaks,max_wound_pressure,max_wound_clot,max_active_fluids,total_fragment_hits,total_fragment_tears,total_fragment_pair_contacts,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,total_post_fracture_joint_corrections,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,max_bone_angular_speed,final_free_fragments,final_spinning_fragments\n";

    const std::vector<Scenario> scenarios = buildScenarios();
    std::vector<ScenarioResult> results;
    results.reserve(scenarios.size());
    bool allContacted = true;
    for (const Scenario& scenario : scenarios) {
        const ScenarioResult result = runScenario(scenario, csv);
        results.push_back(result);
        writeSummary(summaryCsv, scenario, result);
        allContacted = allContacted && (result.tissueContacts > 0 || result.boneContacts > 0);
        std::cout << scenario.name
                  << " tool=" << toolName(scenario.tool)
                  << " tissue_contacts=" << result.tissueContacts
                  << " bone_contacts=" << result.boneContacts
                  << " fractures=" << result.fractures
                  << " max_impact=" << result.maxImpact
                  << " max_point_load=" << result.maxPointLoad
                  << " max_bone_load=" << result.maxBoneLoad
                  << " max_depth=" << result.maxDepth
                  << " fluid=" << result.fluidEmitted
                  << " wound_fluid=" << result.woundFluid
                  << " wounds=" << result.openedWounds
                  << " fragment_hits=" << result.fragmentHits
                  << " fragment_pairs=" << result.fragmentPairContacts
                  << " joint_limits=" << result.postFractureJointCorrections
                  << " max_spin=" << result.maxBoneAngularSpeed
                  << '\n';
    }

    const int reportWarnings = writeTuningReport(report, scenarios, results);
    std::cout << "wrote " << output.string() << '\n';
    std::cout << "wrote " << summaryOutput.string() << '\n';
    std::cout << "wrote " << reportOutput.string() << " warnings=" << reportWarnings << '\n';
    return allContacted ? 0 : 2;
}
