plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlin.serialization)
    `maven-publish`
    signing
}

dependencies {
    implementation(libs.asm.core)
    implementation(libs.asm.tree)
    implementation(libs.asm.commons)
    implementation(libs.asm.util)
    implementation(libs.kotlinx.serialization.json)

    // JUnit 4 RunListener used by OmnivoreTestListener for Android instrumented tests.
    // compileOnly because it's provided by the Android test runner at runtime.
    compileOnly("junit:junit:4.13.2")

    testImplementation(libs.junit.jupiter)
}

tasks.jar {
    manifest {
        attributes(
            "Premain-Class" to "com.jkjamies.omnivore.agent.OmnivoreAgent",
            "Can-Retransform-Classes" to "true",
            "Can-Redefine-Classes" to "true",
        )
    }

    // Create a fat JAR so the agent is self-contained
    from(configurations.runtimeClasspath.get().map { if (it.isDirectory) it else zipTree(it) }) {
        exclude("META-INF/MANIFEST.MF")
        exclude("META-INF/*.SF")
        exclude("META-INF/*.DSA")
        exclude("META-INF/*.RSA")
    }
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE

    archiveBaseName.set("omnivore-agent")
}

// Slim runtime JAR containing only Omnivore classes (no bundled libraries).
// Used as debugImplementation for Android projects where the app APK needs
// OmnivoreRuntime on its classpath but can't use the fat JAR (duplicate class conflicts).
val runtimeJar by tasks.registering(Jar::class) {
    archiveBaseName.set("omnivore-agent-runtime")
    archiveClassifier.set("runtime")
    from(tasks.named("compileKotlin").map { it.outputs })
    from(tasks.named("processResources").map { it.outputs })
    // Only include Omnivore's own classes — not ASM, kotlinx-serialization, etc.
    include("com/jkjamies/omnivore/**")
}

// Ensure the runtime JAR is built alongside the main JAR
tasks.named("assemble") { dependsOn(runtimeJar) }
tasks.named("jar") { finalizedBy(runtimeJar) }

kotlin {
    jvmToolchain(17)
}

// -- Publishing --

java {
    withSourcesJar()
    withJavadocJar()
}

publishing {
    publications {
        create<MavenPublication>("mavenJava") {
            from(components["java"])
            artifactId = "omnivore-agent"

            pom {
                name.set("Omnivore Agent")
                description.set("JVM bytecode instrumentation agent for Omnivore code coverage")
                url.set("https://github.com/jkjamies/coverage-tool")
                licenses {
                    license {
                        name.set("Apache License, Version 2.0")
                        url.set("https://www.apache.org/licenses/LICENSE-2.0")
                    }
                }
                developers {
                    developer {
                        id.set("jkjamies")
                        name.set("jkjamies")
                    }
                }
                scm {
                    url.set("https://github.com/jkjamies/coverage-tool")
                    connection.set("scm:git:git://github.com/jkjamies/coverage-tool.git")
                    developerConnection.set("scm:git:ssh://github.com/jkjamies/coverage-tool.git")
                }
            }
        }
    }

    repositories {
        maven {
            name = "OSSRH"
            val releasesUrl = uri("https://s01.oss.sonatype.org/service/local/staging/deploy/maven2/")
            val snapshotsUrl = uri("https://s01.oss.sonatype.org/content/repositories/snapshots/")
            url = if (version.toString().endsWith("SNAPSHOT")) snapshotsUrl else releasesUrl
            credentials {
                username = providers.environmentVariable("OSSRH_USERNAME").orNull
                    ?: providers.gradleProperty("ossrhUsername").orNull
                password = providers.environmentVariable("OSSRH_PASSWORD").orNull
                    ?: providers.gradleProperty("ossrhPassword").orNull
            }
        }
    }
}

signing {
    // Signing is only required when GPG keys are available (Maven Central / OSSRH)
    val signingKey = providers.environmentVariable("GPG_SIGNING_KEY").orNull
    val signingPassword = providers.environmentVariable("GPG_SIGNING_PASSWORD").orNull
    isRequired = signingKey != null && signingPassword != null
    if (signingKey != null && signingPassword != null) {
        useInMemoryPgpKeys(signingKey, signingPassword)
    }
    sign(publishing.publications["mavenJava"])
}
