plugins {
    kotlin("jvm")
}

dependencies {
    implementation(project(":domain"))

    testImplementation("io.kotest:kotest-runner-junit5:6.2.5")
    testImplementation("io.kotest:kotest-assertions-core:6.2.5")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
}

tasks.withType<Test> {
    useJUnitPlatform()
}
