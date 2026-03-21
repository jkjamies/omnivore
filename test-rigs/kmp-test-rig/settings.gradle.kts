pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
    }

    // Resolve the Omnivore plugin from the local coverage-plugin build
    includeBuild("../../coverage-plugin")
}

@Suppress("UnstableApiUsage")
dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

rootProject.name = "kmp-test-rig"

include(":core")
include(":app")
