plugins {
    kotlin("jvm")
}

dependencies {
    implementation(project(":domain"))

    testImplementation("io.kotest:kotest-runner-junit5:6.1.7")
    testImplementation("io.kotest:kotest-assertions-core:6.1.7")
}

tasks.withType<Test> {
    useJUnitPlatform()
}
