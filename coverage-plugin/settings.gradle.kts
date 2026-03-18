pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
    }
}

@Suppress("UnstableApiUsage")
dependencyResolutionManagement {
    repositories {
        mavenCentral()
        google()
    }
}

rootProject.name = "omnivore-coverage"

include(":omnivore-agent")
include(":omnivore-gradle-plugin")
include(":omnivore-agent-tests")
