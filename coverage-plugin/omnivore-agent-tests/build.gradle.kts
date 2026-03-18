plugins {
    alias(libs.plugins.kotlin.jvm)
}

dependencies {
    testImplementation(project(":omnivore-agent"))
    testImplementation(libs.junit.jupiter)
    testImplementation(libs.asm.core)
    testImplementation(libs.asm.tree)
    testImplementation(libs.asm.util)
}

kotlin {
    jvmToolchain(17)
}
