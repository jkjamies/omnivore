package com.jkjamies.omnivore.gradle.tasks

import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.*
import org.gradle.work.DisableCachingByDefault
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URI

/**
 * Gradle task that uploads Omnivore coverage reports to the dashboard.
 *
 * Finds each `omnivore-report.json` produced by [OmnivoreReportTask] (one per
 * coverage target, in the report dir or a target-named subdirectory) and POSTs
 * each to the dashboard's ingestion endpoint.
 *
 * Usage: `./gradlew omnivoreUpload`
 */
@DisableCachingByDefault(
    because = "POSTs to a dashboard. The result is a remote side effect with no output to cache, " +
        "and a cache hit would silently skip the upload.",
)
abstract class OmnivoreUploadTask : DefaultTask() {

    @get:Internal
    abstract val reportDir: DirectoryProperty

    @get:Input
    abstract val dashboardUrl: Property<String>

    /**
     * API key sent as `X-API-Key`.
     *
     * Deliberately `@Internal` rather than `@Input`: Gradle hashes task inputs
     * into the build cache key and records them in build scans and
     * configuration-cache state, so annotating a credential as an input writes
     * it to disk (and potentially publishes it). This task has no outputs and
     * always runs, so the token was never needed for up-to-date checks anyway.
     */
    @get:Internal
    abstract val authToken: Property<String>

    init {
        reportDir.convention(
            project.layout.buildDirectory.dir("reports/omnivore")
        )
    }

    @TaskAction
    fun upload() {
        val url = dashboardUrl.get().trimEnd('/')
        val dir = reportDir.get().asFile

        // Reports live either directly in the dir (single target) or under a
        // target-named subdirectory (multi-target), so walk the tree.
        val reportFiles = dir.walkTopDown()
            .filter { it.isFile && it.name == "omnivore-report.json" }
            .sortedBy { it.path }
            .toList()

        if (reportFiles.isEmpty()) {
            throw TaskExecutionException(
                this,
                RuntimeException("No report files found in ${dir.absolutePath}. Run omnivoreReport first.")
            )
        }

        val endpoint = "$url/api/v1/ingest/coverage"

        // Coverage reports carry full source paths and, with an API key, a
        // credential. Neither belongs on a plaintext connection to anything but
        // a local dashboard.
        if (endpoint.startsWith("http://") && !isLoopback(url)) {
            val detail = if (authToken.isPresent) " The API key would be sent in clear text." else ""
            logger.warn("Omnivore: uploading to $url over plain HTTP.$detail Use https:// for a remote dashboard.")
        }

        for (file in reportFiles) {
            logger.lifecycle("Uploading ${file.name} to $endpoint")
            uploadFile(file, endpoint)
        }
    }

    private fun isLoopback(url: String): Boolean {
        val host = runCatching { URI(url).host }.getOrNull() ?: return false
        return host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]"
    }

    private fun uploadFile(file: File, endpoint: String) {
        val json = file.readText()
        val connection = URI(endpoint).toURL().openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "POST"
            connection.setRequestProperty("Content-Type", "application/json")
            connection.doOutput = true
            connection.connectTimeout = 15_000
            connection.readTimeout = 30_000

            if (authToken.isPresent) {
                connection.setRequestProperty("X-API-Key", authToken.get())
            }

            connection.outputStream.use { it.write(json.toByteArray()) }

            val responseCode = connection.responseCode
            val responseBody = if (responseCode in 200..299) {
                connection.inputStream.bufferedReader().readText()
            } else {
                connection.errorStream?.bufferedReader()?.readText() ?: "No response body"
            }

            if (responseCode in 200..299) {
                logger.lifecycle("  ${file.name}: success ($responseCode)")
            } else {
                throw IOException("Upload of ${file.name} failed with HTTP $responseCode: $responseBody")
            }
        } finally {
            connection.disconnect()
        }
    }
}
