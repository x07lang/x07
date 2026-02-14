// swift-tools-version:5.9

import PackageDescription

let package = Package(
    name: "x07_tools",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(name: "x07-vz-helper", targets: ["x07_vz_helper"])
    ],
    targets: [
        .executableTarget(
            name: "x07_vz_helper",
            path: "tools/x07-vz-helper",
            exclude: ["entitlements.plist"],
            sources: ["main.swift"],
            linkerSettings: [
                .linkedFramework("Virtualization")
            ]
        )
    ]
)
