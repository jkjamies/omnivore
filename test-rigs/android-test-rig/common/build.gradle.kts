plugins {
    kotlin("jvm")
}

dependencies {
    implementation(project(":domain"))

    testImplementation("io.kotest:kotest-runner-junit5:6.2.5")
    testImplementation("io.kotest:kotest-assertions-core:6.2.5")
}

tasks.withType<Test> {
    useJUnitPlatform()
}
