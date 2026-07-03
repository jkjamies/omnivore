plugins {
    kotlin("jvm") version "2.1.10" apply false
    id("io.github.jkjamies.omnivore")
    // Kover produces a JaCoCo-compatible XML report (koverXmlReport), giving this
    // rig a second JVM_UNIT coverage source that the dashboard tracks as its own
    // (target, source) series alongside the Omnivore agent. It is OPT-IN and kept
    // off by default: applying Kover instruments the `test` task, and the default
    // `omnivoreReport` flow (and CI) already instruments those same tests with the
    // Omnivore agent. Enabling it only on request keeps the two off each other.
    id("org.jetbrains.kotlinx.kover") version "0.9.8" apply false
}

// Enable with `-Pomnivore.kover` (presence, any/no value):
//   ./gradlew koverXmlReport -Pomnivore.kover   →  build/reports/kover/report.xml
val koverEnabled = providers.gradleProperty("omnivore.kover").isPresent

omnivore {
    reports {
        json { enabled.set(true) }
        html { enabled.set(true) }
        markdown { enabled.set(true) }
    }
    dependencies {
        enabled.set(true)
    }
    dashboard {
        url.set(providers.gradleProperty("omnivore.dashboard.url").orElse("http://localhost:3000"))
    }
}

subprojects {
    apply(plugin = "org.jetbrains.kotlin.jvm")
    // Instrument each module so the aggregated koverXmlReport includes its classes.
    if (koverEnabled) {
        apply(plugin = "org.jetbrains.kotlinx.kover")
    }

    configure<org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension> {
        jvmToolchain(17)
    }

    dependencies {
        "testImplementation"("io.kotest:kotest-runner-junit5:6.1.7")
        "testImplementation"("io.kotest:kotest-assertions-core:6.1.7")
    }

    tasks.withType<Test> {
        useJUnitPlatform()
    }
}

// Aggregate every subproject's coverage into the root koverXmlReport.
if (koverEnabled) {
    apply(plugin = "org.jetbrains.kotlinx.kover")
    dependencies {
        add("kover", project(":core"))
        add("kover", project(":app"))
    }
}
