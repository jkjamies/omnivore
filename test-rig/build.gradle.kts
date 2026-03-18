plugins {
    kotlin("jvm") version "2.1.10" apply false
    id("io.github.jkjamies.omnivore")
}

omnivore {
    composeFilter {
        enabled.set(false)
    }
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
        "testImplementation"("org.junit.jupiter:junit-jupiter:5.11.4")
    }

    tasks.withType<Test> {
        useJUnitPlatform()
    }
}
