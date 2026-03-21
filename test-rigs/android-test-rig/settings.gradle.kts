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
        google()
    }
}

// Also include for dependency resolution — allows the plugin to resolve the
// omnivore-agent runtime JAR via Gradle's artifact substitution (composite build).
// When the plugin is published, this line is unnecessary (resolves from Maven Central).
includeBuild("../../coverage-plugin")

rootProject.name = "android-test-rig"

include(":domain")
include(":data")
include(":common")
include(":app")
