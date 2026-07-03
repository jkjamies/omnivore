plugins {
    id("com.android.application")
    kotlin("android")
    kotlin("plugin.compose")
}

// AGP bundles JaCoCo. Enabling unit-test coverage on the debug variant makes
// `createDebugUnitTestCoverageReport` emit a JaCoCo-compatible XML report — a
// convenient way to exercise the dashboard's JaCoCo ingestion path. It is
// OPT-IN via `-Pomnivore.jacoco` and off by default: it instruments the same
// unit tests the Omnivore agent already covers during `omnivoreReport`, so
// enabling it only on request keeps the two off each other (and leaves CI
// untouched). Enable with:
//   ./gradlew :app:createDebugUnitTestCoverageReport -Pomnivore.jacoco
//   → app/build/reports/coverage/test/debug/report.xml
val jacocoEnabled = providers.gradleProperty("omnivore.jacoco").isPresent

android {
    namespace = "com.example.android.testrig"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.example.android.testrig"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    buildTypes {
        debug {
            // Off unless -Pomnivore.jacoco is passed (see note at top of file).
            enableUnitTestCoverage = jacocoEnabled
        }
    }
}

dependencies {
    implementation(project(":domain"))
    implementation(project(":data"))
    implementation(project(":common"))

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.8.7")

    implementation(platform("androidx.compose:compose-bom:2024.12.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")

    testImplementation("io.kotest:kotest-runner-junit5:6.1.7")
    testImplementation("io.kotest:kotest-assertions-core:6.1.7")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.10.1")

    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test:rules:1.6.1")
}

tasks.withType<Test> {
    useJUnitPlatform()
}
