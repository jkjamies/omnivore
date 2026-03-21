plugins {
    kotlin("jvm") version "2.1.10" apply false
    id("io.github.jkjamies.omnivore")
}

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
