cmake_minimum_required(VERSION 3.24)

project(RealisticPhysics LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

add_library(realistic_physics_core
    src/cpp/simulation.cpp
)

target_include_directories(realistic_physics_core
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp
)

target_compile_features(realistic_physics_core PUBLIC cxx_std_20)

if(MSVC)
    target_compile_options(realistic_physics_core PRIVATE /W4 /permissive-)
else()
    target_compile_options(realistic_physics_core PRIVATE -Wall -Wextra -Wpedantic)
endif()

add_executable(realistic_physics_tests
    tests/simulation_tests.cpp
)

target_link_libraries(realistic_physics_tests PRIVATE realistic_physics_core)

add_executable(realistic_physics_diagnostics
    tools/anatomy_diagnostics.cpp
)

target_link_libraries(realistic_physics_diagnostics PRIVATE realistic_physics_core)

add_executable(realistic_physics_strike_scenarios
    tools/strike_scenarios.cpp
)

target_link_libraries(realistic_physics_strike_scenarios PRIVATE realistic_physics_core)

if(WIN32)
    add_executable(realistic_physics WIN32
        src/cpp/win32_main.cpp
    )

    target_link_libraries(realistic_physics PRIVATE realistic_physics_core)

    set_target_properties(realistic_physics PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY_DEBUG "${CMAKE_CURRENT_SOURCE_DIR}"
        RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_CURRENT_SOURCE_DIR}"
        RUNTIME_OUTPUT_DIRECTORY_RELWITHDEBINFO "${CMAKE_CURRENT_SOURCE_DIR}"
        RUNTIME_OUTPUT_DIRECTORY_MINSIZEREL "${CMAKE_CURRENT_SOURCE_DIR}"
    )

    if(MSVC)
        target_compile_options(realistic_physics PRIVATE /W4 /permissive-)
    endif()
endif()
