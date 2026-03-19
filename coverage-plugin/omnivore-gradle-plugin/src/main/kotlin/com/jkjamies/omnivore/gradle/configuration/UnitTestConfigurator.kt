package com.jkjamies.omnivore.gradle.configuration

import com.jkjamies.omnivore.gradle.OmnivoreExtension
import org.gradle.api.Project
import org.gradle.api.tasks.testing.Test
import java.io.File

/**
 * Configures unit test tasks to use the Omnivore coverage agent.
 *
 * Adds -javaagent:omnivore-agent.jar to the JVM arguments of all Test tasks,
 * so that coverage probes are active during unit test execution.
 */
object UnitTestConfigurator {

    fun configure(project: Project, extension: OmnivoreExtension) {
        // Write coverage data to the root project's build/omnivore/ so the
        // report task can find data from all subprojects in one place.
        val rootProject = project.rootProject

        project.tasks.withType(Test::class.java).configureEach { testTask ->
            val agentJar = resolveAgentJar()

            if (agentJar != null) {
                val destFile = rootProject.layout.buildDirectory
                    .file("omnivore/${project.path.replace(':', '/')}/${testTask.name}/coverage.omnivore")
                    .get().asFile

                val agentArgs = buildString {
                    append("destfile=${destFile.absolutePath}")
                    val includes = extension.includes.get()
                    if (includes.isNotEmpty()) {
                        append(",includes=${includes.joinToString(":")}")
                    }
                    val excludes = extension.excludes.get()
                    if (excludes.isNotEmpty()) {
                        append(",excludes=${excludes.joinToString(":")}")
                    }
                    append(",compose=true")
                }

                testTask.jvmArgs("-javaagent:${agentJar.absolutePath}=$agentArgs")
            } else {
                project.logger.warn("Omnivore: Could not locate omnivore-agent.jar on the plugin classpath.")
            }
        }
    }

    /**
     * Resolve the agent JAR from this plugin's own classpath.
     *
     * The omnivore-gradle-plugin depends on omnivore-agent, so the agent JAR
     * is on the plugin classloader's classpath. We find it by locating the
     * class file for OmnivoreAgent and extracting the JAR path.
     */
    private fun resolveAgentJar(): File? {
        val agentClass = "com.jkjamies.omnivore.agent.OmnivoreAgent"
        return try {
            val classResource = Class.forName(agentClass)
                .protectionDomain
                .codeSource
                ?.location
                ?.toURI()
            classResource?.let { File(it) }?.takeIf { it.exists() }
        } catch (_: Exception) {
            null
        }
    }
}
