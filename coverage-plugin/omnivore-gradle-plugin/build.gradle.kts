plugins {
    alias(libs.plugins.kotlin.jvm)
    `java-library`
    `java-gradle-plugin`
    `maven-publish`
    signing
    alias(libs.plugins.gradle.plugin.publish)
}

dependencies {
    api(project(":omnivore-agent"))

    // Gradle API is provided by java-gradle-plugin
    compileOnly(libs.agp)
    compileOnly(libs.kotlin.gradle.plugin)

    testImplementation(libs.junit.jupiter)
}

gradlePlugin {
    website.set("https://github.com/jkjamies/coverage-tool")
    vcsUrl.set("https://github.com/jkjamies/coverage-tool.git")

    plugins {
        create("omnivore") {
            id = "io.github.jkjamies.omnivore"
            displayName = "Omnivore Coverage"
            description = "Compose-aware code coverage for Android, Kotlin, and KMP projects"
            implementationClass = "com.jkjamies.omnivore.gradle.OmnivorePlugin"
            tags.set(listOf("coverage", "kotlin", "android", "compose", "kmp", "testing"))
        }
    }
}

// Generate a version properties file so the plugin knows its own coordinates at runtime.
// This mirrors JaCoCo's approach where the plugin resolves its agent via Gradle configurations
// rather than fragile classloader-based JAR scanning.
val generateVersionProps by tasks.registering {
    val outputDir = layout.buildDirectory.dir("generated/omnivore-version")
    val groupId = project.group.toString()
    val artifactVersion = project.version.toString()
    outputs.dir(outputDir)
    doLast {
        val propsFile = outputDir.get().file("omnivore-version.properties").asFile
        propsFile.parentFile.mkdirs()
        propsFile.writeText(
            "group=$groupId\nartifactId=omnivore-agent\nversion=$artifactVersion\n"
        )
    }
}
sourceSets.main { resources.srcDir(generateVersionProps) }

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
        // The java-gradle-plugin automatically creates a "pluginMaven" publication.
        // Configure it with POM metadata.
        withType<MavenPublication> {
            pom {
                name.set("Omnivore Gradle Plugin")
                description.set("Gradle plugin for Omnivore code coverage — Compose-aware coverage for Android, Kotlin, and KMP")
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
    // Signing is only required for Maven Central (OSSRH), not Gradle Plugin Portal
    val signingKey = providers.environmentVariable("GPG_SIGNING_KEY").orNull
    val signingPassword = providers.environmentVariable("GPG_SIGNING_PASSWORD").orNull
    isRequired = signingKey != null && signingPassword != null
    if (signingKey != null && signingPassword != null) {
        useInMemoryPgpKeys(signingKey, signingPassword)
    }
    sign(publishing.publications)
}
