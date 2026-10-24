plugins {
    kotlin("jvm")
}

dependencies {
    implementation(project(":domain"))

    testImplementation("io.kotest:kotest-runner-junit5:6.2.6")
    testImplementation("io.kotest:kotest-assertions-core:6.2.6")
}

tasks.withType<Test> {
    useJUnitPlatform()
}
