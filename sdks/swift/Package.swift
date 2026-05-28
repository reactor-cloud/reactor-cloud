// swift-tools-version: 5.10

import PackageDescription

let package = Package(
    name: "Reactor",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
        .tvOS(.v17),
        .watchOS(.v10),
        .visionOS(.v1)
    ],
    products: [
        // Umbrella library - the main entry point for consumers
        .library(
            name: "Reactor",
            targets: ["Reactor"]
        ),
        // Individual capability libraries for granular imports
        .library(
            name: "ReactorShared",
            targets: ["ReactorShared"]
        ),
        .library(
            name: "ReactorAuth",
            targets: ["ReactorAuth"]
        ),
        .library(
            name: "ReactorData",
            targets: ["ReactorData"]
        ),
        .library(
            name: "ReactorStorage",
            targets: ["ReactorStorage"]
        ),
        .library(
            name: "ReactorFunctions",
            targets: ["ReactorFunctions"]
        ),
        .library(
            name: "ReactorJobs",
            targets: ["ReactorJobs"]
        ),
        .library(
            name: "ReactorSites",
            targets: ["ReactorSites"]
        ),
        .library(
            name: "ReactorRealtime",
            targets: ["ReactorRealtime"]
        ),
        .library(
            name: "ReactorAnalytics",
            targets: ["ReactorAnalytics"]
        ),
        .library(
            name: "ReactorAI",
            targets: ["ReactorAI"]
        ),
    ],
    targets: [
        // MARK: - Foundation Targets
        
        .target(
            name: "ReactorShared",
            dependencies: [],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorSharedTests",
            dependencies: ["ReactorShared"]
        ),
        
        // MARK: - Capability Targets
        
        .target(
            name: "ReactorAuth",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorAuthTests",
            dependencies: ["ReactorAuth"]
        ),
        
        .target(
            name: "ReactorData",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorDataTests",
            dependencies: ["ReactorData"]
        ),
        
        .target(
            name: "ReactorStorage",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorStorageTests",
            dependencies: ["ReactorStorage"]
        ),
        
        .target(
            name: "ReactorFunctions",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorFunctionsTests",
            dependencies: ["ReactorFunctions"]
        ),
        
        .target(
            name: "ReactorJobs",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorJobsTests",
            dependencies: ["ReactorJobs"]
        ),
        
        .target(
            name: "ReactorSites",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorSitesTests",
            dependencies: ["ReactorSites"]
        ),
        
        .target(
            name: "ReactorRealtime",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorRealtimeTests",
            dependencies: ["ReactorRealtime"]
        ),
        
        .target(
            name: "ReactorAnalytics",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorAnalyticsTests",
            dependencies: ["ReactorAnalytics"]
        ),
        
        .target(
            name: "ReactorAI",
            dependencies: ["ReactorShared"],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorAITests",
            dependencies: ["ReactorAI"]
        ),
        
        // MARK: - Umbrella Target
        
        .target(
            name: "Reactor",
            dependencies: [
                "ReactorShared",
                "ReactorAuth",
                "ReactorData",
                "ReactorStorage",
                "ReactorFunctions",
                "ReactorJobs",
                "ReactorSites",
                "ReactorRealtime",
                "ReactorAnalytics",
                "ReactorAI",
            ],
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "ReactorTests",
            dependencies: ["Reactor"]
        ),
        
        // MARK: - Examples
        
        .executableTarget(
            name: "ReactorSampleCLI",
            dependencies: ["Reactor"],
            path: "Examples/ReactorSampleCLI"
        ),
    ]
)
