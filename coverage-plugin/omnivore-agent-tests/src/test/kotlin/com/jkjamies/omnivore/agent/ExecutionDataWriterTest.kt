package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.ExecutionDataWriter
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import java.io.DataInputStream
import java.io.File
import java.io.FileInputStream

class ExecutionDataWriterTest {

    @TempDir
    lateinit var tempDir: File

    @Test
    fun `writes execution data to file`() {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(42L, "com/example/Foo", 5)
        probes[0] = true
        probes[2] = true
        probes[4] = true

        val outputFile = File(tempDir, "coverage.exec")
        ExecutionDataWriter.write(outputFile, store)

        assertTrue(outputFile.exists())
        assertTrue(outputFile.length() > 0)

        // Read it back and verify
        DataInputStream(FileInputStream(outputFile).buffered()).use { input ->
            // Header
            val magic = ByteArray(8)
            input.readFully(magic)
            assertEquals("OMNIVORE", String(magic))

            val version = input.readShort()
            assertEquals(1.toShort(), version)

            // Number of classes
            val classCount = input.readInt()
            assertEquals(1, classCount)

            // Class data
            val classId = input.readLong()
            assertEquals(42L, classId)

            val className = input.readUTF()
            assertEquals("com/example/Foo", className)

            val probeCount = input.readInt()
            assertEquals(5, probeCount)

            // Read packed bits (ceil(5/8) = 1 byte)
            val packed = ByteArray(1)
            input.readFully(packed)

            // Verify bits: probes 0, 2, 4 should be set
            // Bit 0 (1), Bit 2 (4), Bit 4 (16) = 0b00010101 = 21
            assertEquals(21, packed[0].toInt() and 0xFF)
        }
    }

    @Test
    fun `writes multiple classes`() {
        val store = ExecutionDataStore()
        store.getOrCreateProbes(1L, "com/example/Foo", 3)
        store.getOrCreateProbes(2L, "com/example/Bar", 7)

        val outputFile = File(tempDir, "coverage.exec")
        ExecutionDataWriter.write(outputFile, store)

        DataInputStream(FileInputStream(outputFile).buffered()).use { input ->
            val magic = ByteArray(8)
            input.readFully(magic)
            assertEquals("OMNIVORE", String(magic))
            input.readShort() // version

            val classCount = input.readInt()
            assertEquals(2, classCount)
        }
    }

    @Test
    fun `creates parent directories if needed`() {
        val store = ExecutionDataStore()
        store.getOrCreateProbes(1L, "com/example/Foo", 3)

        val outputFile = File(tempDir, "nested/deep/coverage.exec")
        ExecutionDataWriter.write(outputFile, store)

        assertTrue(outputFile.exists())
    }

    @Test
    fun `handles empty data store`() {
        val store = ExecutionDataStore()
        val outputFile = File(tempDir, "coverage.exec")
        ExecutionDataWriter.write(outputFile, store)

        DataInputStream(FileInputStream(outputFile).buffered()).use { input ->
            val magic = ByteArray(8)
            input.readFully(magic)
            assertEquals("OMNIVORE", String(magic))
            input.readShort() // version

            val classCount = input.readInt()
            assertEquals(0, classCount)
        }
    }
}
