package com.jkjamies.omnivore.gradle.tasks

import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.*
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URI

/**
 * Gradle task that uploads Omnivore coverage reports to the dashboard.
 *
 * Finds the `omnivore-report.json` produced by [OmnivoreReportTask] and POSTs
 * it to the dashboard's ingestion endpoint.
 *
 * Usage: `./gradlew omnivoreUpload`
 */
abstract class OmnivoreUploadTask : DefaultTask() {

    @get:Internal
    abstract val reportDir: DirectoryProperty

    @get:Input
    abstract val dashboardUrl: Property<String>

    @get:Input
    @get:Optional
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

        val reportFiles = dir.listFiles()
            ?.filter { it.name == "omnivore-report.json" }
            ?.sorted()
            ?: emptyList()

        if (reportFiles.isEmpty()) {
            throw TaskExecutionException(
                this,
                RuntimeException("No report files found in ${dir.absolutePath}. Run omnivoreReport first.")
            )
        }

        val endpoint = "$url/api/v1/ingest/coverage"

        for (file in reportFiles) {
            logger.lifecycle("Uploading ${file.name} to $endpoint")
            uploadFile(file, endpoint)
        }
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
