// swift-tools-version:5.9

import PackageDescription

let package = Package(
    name: "x07_vz_helper",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(name: "x07-vz-helper", targets: ["x07_vz_helper"])
    ],
    targets: [
        .executableTarget(
            name: "x07_vz_helper",
            path: ".",
            exclude: ["entitlements.plist", "Package.swift"],
            sources: ["main.swift"],
            linkerSettings: [
                .linkedFramework("Virtualization")
            ]
        )
    ]
)
