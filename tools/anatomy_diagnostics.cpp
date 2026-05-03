#include "simulation.hpp"

#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <string>

namespace {

void writeTriangle(std::ostream& out, const rp::World& world, const rp::Triangle& triangle, const char* fill, double opacity) {
    const auto& points = world.points();
    const rp::Vec2 a = points[triangle.a].position;
    const rp::Vec2 b = points[triangle.b].position;
    const rp::Vec2 c = points[triangle.c].position;
    out << "<polygon points=\""
        << a.x << "," << a.y << " "
        << b.x << "," << b.y << " "
        << c.x << "," << c.y
        << "\" fill=\"" << fill << "\" fill-opacity=\"" << opacity
        << "\" stroke=\"" << fill << "\" stroke-opacity=\"0.28\" stroke-width=\"1\"/>\n";
}

rp::Vec2 bonePoint(const rp::BoneSegment& bone, double t) {
    return {
        bone.a.x + (bone.b.x - bone.a.x) * t,
        bone.a.y + (bone.b.y - bone.a.y) * t,
    };
}

bool writeSvg(const std::filesystem::path& path, const rp::World& world, double width, double height) {
    if (path.has_parent_path()) {
        std::filesystem::create_directories(path.parent_path());
    }

    std::ofstream out(path);
    if (!out) {
        return false;
    }

    out << std::fixed << std::setprecision(2);
    out << "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"" << width << "\" height=\"" << height
        << "\" viewBox=\"0 0 " << width << " " << height << "\">\n";
    out << "<rect width=\"100%\" height=\"100%\" fill=\"#181818\"/>\n";

    for (const rp::Triangle& triangle : world.triangles()) {
        if (triangle.layer == rp::TissueLayer::Skin && world.triangleAlive(triangle)) {
            writeTriangle(out, world, triangle, "#9b705e", 0.22);
        }
    }
    for (const rp::Triangle& triangle : world.triangles()) {
        if (triangle.layer == rp::TissueLayer::Muscle && world.triangleAlive(triangle)) {
            writeTriangle(out, world, triangle, "#b62e3a", 0.50);
        }
    }

    for (const rp::BoneAttachment& attachment : world.boneAttachments()) {
        if (attachment.broken || attachment.bone >= world.bones().size() || attachment.point >= world.points().size()) {
            continue;
        }
        const rp::BoneSegment& bone = world.bones()[attachment.bone];
        const rp::Vec2 anchor = bonePoint(bone, attachment.t);
        const rp::Vec2 point = world.points()[attachment.point].position;
        out << "<line x1=\"" << anchor.x << "\" y1=\"" << anchor.y
            << "\" x2=\"" << point.x << "\" y2=\"" << point.y
            << "\" stroke=\"#57a6ff\" stroke-opacity=\"0.16\" stroke-width=\"1\"/>\n";
    }

    for (const rp::BoneJoint& joint : world.boneJoints()) {
        if (joint.broken || joint.a >= world.bones().size() || joint.b >= world.bones().size()) {
            continue;
        }
        const rp::Vec2 a = bonePoint(world.bones()[joint.a], joint.tA);
        const rp::Vec2 b = bonePoint(world.bones()[joint.b], joint.tB);
        out << "<line x1=\"" << a.x << "\" y1=\"" << a.y
            << "\" x2=\"" << b.x << "\" y2=\"" << b.y
            << "\" stroke=\"#ffd36a\" stroke-opacity=\"0.42\" stroke-width=\"2\"/>\n";
        out << "<circle cx=\"" << a.x << "\" cy=\"" << a.y << "\" r=\"3\" fill=\"#ffd36a\" fill-opacity=\"0.82\"/>\n";
        out << "<circle cx=\"" << b.x << "\" cy=\"" << b.y << "\" r=\"3\" fill=\"#ffd36a\" fill-opacity=\"0.82\"/>\n";
    }

    for (const rp::BoneSegment& bone : world.bones()) {
        out << "<line x1=\"" << bone.a.x << "\" y1=\"" << bone.a.y
            << "\" x2=\"" << bone.b.x << "\" y2=\"" << bone.b.y
            << "\" stroke=\"" << (bone.fractured ? "#fff3d6" : "#e5d5aa")
            << "\" stroke-width=\"" << bone.radius * 1.8
            << "\" stroke-linecap=\"round\"/>\n";
    }

    for (const rp::BoneSegment& bone : world.bones()) {
        constexpr int samples = 16;
        for (int i = 0; i < samples; ++i) {
            const double t = static_cast<double>(i) / static_cast<double>(samples - 1);
            const rp::Vec2 sample = bonePoint(bone, t);
            const bool insideSkin = rp::pointInsideLayer(world, sample, rp::TissueLayer::Skin);
            out << "<circle cx=\"" << sample.x << "\" cy=\"" << sample.y << "\" r=\"2.5\" fill=\""
                << (insideSkin ? "#59d36d" : "#ff4f42") << "\"/>\n";
        }
    }

    out << "</svg>\n";
    return true;
}

} // namespace

int main(int argc, char** argv) {
    constexpr double width = 1280.0;
    constexpr double height = 720.0;
    const std::filesystem::path output = argc > 1 ? std::filesystem::path(argv[1]) : std::filesystem::path("output/anatomy_debug.svg");

    const rp::World world = rp::createLayeredBody(width, height);
    const rp::AnatomyValidation validation = rp::validateAnatomy(world);

    std::cout << "points=" << world.points().size()
              << " springs=" << world.springs().size()
              << " triangles=" << world.triangles().size()
              << " bones=" << world.bones().size()
              << " bone_joints=" << world.boneJoints().size()
              << " bone_attachments=" << world.boneAttachments().size()
              << '\n';
    std::cout << "skin_points=" << validation.skinPoints
              << " muscle_points=" << validation.musclePoints
              << " bone_samples=" << validation.boneSamples
              << " bone_samples_outside_skin=" << validation.boneSamplesOutsideSkin
              << " bone_samples_outside_muscle=" << validation.boneSamplesOutsideMuscle
              << '\n';

    if (!writeSvg(output, world, width, height)) {
        std::cerr << "failed to write " << output.string() << '\n';
        return 1;
    }

    std::cout << "wrote " << output.string() << '\n';
    return validation.boneSamplesOutsideSkin == 0 ? 0 : 2;
}
