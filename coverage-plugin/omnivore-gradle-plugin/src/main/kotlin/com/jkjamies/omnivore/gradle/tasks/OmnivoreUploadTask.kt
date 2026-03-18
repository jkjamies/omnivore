package com.jkjamies.omnivore.gradle.tasks

import org.gradle.api.DefaultTask
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.*
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URI

/**
 * Gradle task that uploads an Omnivore coverage report to the dashboard.
 *
 * Reads the JSON report produced by [OmnivoreReportTask] and POSTs it
 * to the dashboard's ingestion endpoint.
 *
 * Usage: `./gradlew omnivoreUpload`
 */
abstract class OmnivoreUploadTask : DefaultTask() {

    @get:InputFile
    abstract val reportFile: RegularFileProperty

    @get:Input
    abstract val dashboardUrl: Property<String>

    @get:Input
    @get:Optional
    abstract val authToken: Property<String>

    init {
        reportFile.convention(
            project.layout.buildDirectory.file("reports/omnivore/omnivore-report.json")
        )
    }

    @TaskAction
    fun upload() {
        val url = dashboardUrl.get().trimEnd('/')
        val file = reportFile.get().asFile

        if (!file.exists()) {
            throw TaskExecutionException(
                this,
                RuntimeException("Report file not found: ${file.absolutePath}. Run omnivoreReport first.")
            )
        }

        val json = file.readText()
        val endpoint = "$url/api/v1/ingest/coverage"

        logger.lifecycle("Uploading coverage report to $endpoint")

        val connection = URI(endpoint).toURL().openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "POST"
            connection.setRequestProperty("Content-Type", "application/json")
            connection.doOutput = true
            connection.connectTimeout = 15_000
            connection.readTimeout = 30_000

            if (authToken.isPresent) {
                connection.setRequestProperty("Authorization", "Bearer ${authToken.get()}")
            }

            connection.outputStream.use { it.write(json.toByteArray()) }

            val responseCode = connection.responseCode
            val responseBody = if (responseCode in 200..299) {
                connection.inputStream.bufferedReader().readText()
            } else {
                connection.errorStream?.bufferedReader()?.readText() ?: "No response body"
            }

            if (responseCode in 200..299) {
                logger.lifecycle("Upload successful ($responseCode): $responseBody")
            } else {
                throw IOException("Upload failed with HTTP $responseCode: $responseBody")
            }
        } finally {
            connection.disconnect()
        }
    }
}
