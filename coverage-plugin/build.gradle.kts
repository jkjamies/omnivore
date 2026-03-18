plugins {
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.kotlin.serialization) apply false
}

allprojects {
    group = "io.github.jkjamies"
    version = "0.1.0-SNAPSHOT"
}

subprojects {
    tasks.withType<Test> {
        useJUnitPlatform()
    }
}

// Shared POM configuration for published modules
ext["pomName"] = "Omnivore Coverage"
ext["pomDescription"] = "Compose-aware code coverage for Android, Kotlin, and KMP projects"
ext["pomUrl"] = "https://github.com/jkjamies/coverage-tool"
ext["pomLicense"] = "Apache-2.0"
ext["pomScmUrl"] = "https://github.com/jkjamies/coverage-tool.git"
