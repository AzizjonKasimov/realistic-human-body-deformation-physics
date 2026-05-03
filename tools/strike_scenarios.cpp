#include "simulation.hpp"

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <string>
#include <vector>

namespace {

struct Scenario {
    const char* name = "";
    rp::Vec2 start;
    rp::Vec2 end;
    int windupFrames = 12;
    int strikeFrames = 34;
    int settleFrames = 42;
    double power = 4.0;
};

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
    double maxImpact = 0.0;
    double maxBoneLoad = 0.0;
    double maxPointLoad = 0.0;
    double maxDepth = 0.0;
};

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
    return input;
}

void writeFrame(std::ostream& out, const Scenario& scenario, int frame, const rp::World& world) {
    const rp::ContactDebug& debug = world.debug();
    out << scenario.name << ','
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
        << world.stats().fracturedBones << '\n';
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
        writeFrame(csv, scenario, frame, world);
    }

    result.skinTears = world.stats().brokenSkin;
    result.muscleTears = world.stats().brokenMuscle;
    result.detachments = world.stats().brokenAttachments;
    result.boneDetachments = world.stats().brokenBoneAttachments;
    result.boneJointBreaks = world.stats().brokenBoneJoints;
    result.boneFractures = world.stats().fracturedBones;
    result.finalBones = static_cast<int>(world.bones().size());
    return result;
}

void writeSummary(std::ostream& out, const Scenario& scenario, const ScenarioResult& result) {
    out << scenario.name << ','
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
        << result.finalBones << '\n';
}

} // namespace

int main(int argc, char** argv) {
    const std::filesystem::path output = argc > 1
        ? std::filesystem::path(argv[1])
        : std::filesystem::path("output/strike_scenarios.csv");
    std::filesystem::path summaryOutput = output;
    summaryOutput.replace_filename("strike_summary.csv");
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

    csv << std::fixed << std::setprecision(3);
    summaryCsv << std::fixed << std::setprecision(3);
    csv << "scenario,frame,x,y,speed,impact,tissue_contacts,bone_contacts,max_depth,max_point_load,max_bone_load,step_fractures,skin_tears,muscle_tears,detachments,bone_detachments,bone_joint_breaks,bone_fractures\n";
    summaryCsv << "scenario,total_tissue_contacts,total_bone_contacts,total_step_fractures,max_impact,max_point_load,max_bone_load,max_depth,skin_tears,muscle_tears,detachments,bone_detachments,bone_joint_breaks,bone_fractures,final_bones\n";

    const std::vector<Scenario> scenarios{
        {"torso_blunt", {545.0, 336.0}, {704.0, 336.0}, 12, 34, 48, 4.0},
        {"left_shoulder_blunt", {460.0, 238.0}, {592.0, 258.0}, 12, 30, 44, 3.0},
        {"hip_drive", {548.0, 420.0}, {692.0, 430.0}, 12, 32, 44, 4.0},
    };

    bool allContacted = true;
    for (const Scenario& scenario : scenarios) {
        const ScenarioResult result = runScenario(scenario, csv);
        writeSummary(summaryCsv, scenario, result);
        allContacted = allContacted && (result.tissueContacts > 0 || result.boneContacts > 0);
        std::cout << scenario.name
                  << " tissue_contacts=" << result.tissueContacts
                  << " bone_contacts=" << result.boneContacts
                  << " fractures=" << result.fractures
                  << " max_impact=" << result.maxImpact
                  << " max_point_load=" << result.maxPointLoad
                  << " max_bone_load=" << result.maxBoneLoad
                  << " max_depth=" << result.maxDepth
                  << '\n';
    }

    std::cout << "wrote " << output.string() << '\n';
    std::cout << "wrote " << summaryOutput.string() << '\n';
    return allContacted ? 0 : 2;
}
