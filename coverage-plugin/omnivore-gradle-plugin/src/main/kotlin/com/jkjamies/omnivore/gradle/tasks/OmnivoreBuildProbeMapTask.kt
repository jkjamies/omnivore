package com.jkjamies.omnivore.gradle.tasks

import com.jkjamies.omnivore.agent.instrumentation.EmbeddedProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeMapWriter
import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.tasks.CacheableTask
import org.gradle.api.tasks.Classpath
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.TaskAction
import java.io.File
import java.util.zip.ZipFile

/**
 * Writes the build-time probe map for one Android variant by reading the
 * probe maps embedded in that variant's instrumented classes.
 *
 * [instrumentedClasses] is everything AGP's ASM transform produced for the
 * variant: the module's own classes (the `transform<Variant>ClassesWithAsm`
 * outputs) and its runtime dependencies (AGP's ASM artifact transform). Reading
 * the outputs rather than collecting during the transform is what makes the map
 * complete on incremental, cached, and fresh-daemon builds alike — see
 * `EmbeddedProbeMap`.
 */
@CacheableTask
abstract class OmnivoreBuildProbeMapTask : DefaultTask() {

    @get:Classpath
    abstract val instrumentedClasses: ConfigurableFileCollection

    @get:OutputFile
    abstract val probeMapFile: RegularFileProperty

    @TaskAction
    fun write() {
        val probeMap = ProbeMap()
        for (root in instrumentedClasses.files) {
            when {
                root.isDirectory -> root.walkTopDown()
                    .filter { it.isFile && it.name.endsWith(".class") }
                    .forEach { addClass(probeMap, it.readBytes(), it.path) }
                root.isFile && root.name.endsWith(".jar") -> ZipFile(root).use { zip ->
                    for (entry in zip.entries()) {
                        if (entry.isDirectory || !entry.name.endsWith(".class")) continue
                        addClass(probeMap, zip.getInputStream(entry).use { it.readBytes() }, "${root.name}!${entry.name}")
                    }
                }
            }
        }

        val output = probeMapFile.get().asFile
        // Written even when empty, so a stale map from an earlier build can
        // never outlive the classes it described.
        ProbeMapWriter.write(output, probeMap)
        val classCount = probeMap.getAllClassMaps().size
        if (classCount == 0) {
            logger.warn("Omnivore: no instrumented classes found for ${output.name}; instrumented coverage will be empty")
        } else {
            logger.info("Omnivore: wrote build-time probe map for $classCount classes to $output")
        }
    }

    private fun addClass(probeMap: ProbeMap, bytes: ByteArray, origin: String) {
        val classMap = try {
            EmbeddedProbeMap.read(bytes)
        } catch (e: Exception) {
            logger.warn("Omnivore: could not read embedded probe map from $origin: ${e.message}")
            null
        } ?: return
        probeMap.putClassMap(classMap)
    }

    companion object {
        /** Name of the probe map file for [variant] of [projectPath], unique across modules. */
        fun fileName(projectPath: String, variant: String): String {
            val module = projectPath.trim(':').replace(':', '-').ifEmpty { "root" }
            return "build-time-$module-$variant.probes"
        }
    }
}
