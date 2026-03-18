package com.jkjamies.omnivore.agent.runtime

import java.io.DataInputStream
import java.io.File
import java.io.FileInputStream

/**
 * Reads execution data from .omnivore binary files.
 * Used by the reporter to reconstruct coverage data after test execution.
 */
object ExecutionDataReader {

    private const val MAGIC = "OMNIVORE"

    fun read(file: File): ExecutionDataStore {
        val store = ExecutionDataStore()

        DataInputStream(FileInputStream(file).buffered()).use { input ->
            // Header
            val magic = ByteArray(8)
            input.readFully(magic)
            check(String(magic) == MAGIC) { "Invalid execution data file: bad magic" }

            val version = input.readShort()
            check(version.toInt() == 1) { "Unsupported version: $version" }

            val classCount = input.readInt()

            for (i in 0 until classCount) {
                val classId = input.readLong()
                val className = input.readUTF()
                val probeCount = input.readInt()

                // Read packed probe bits
                val packedSize = (probeCount + 7) / 8
                val packed = ByteArray(packedSize)
                input.readFully(packed)

                // Unpack into boolean array
                val probes = store.getOrCreateProbes(classId, className, probeCount)
                for (j in 0 until probeCount) {
                    if ((packed[j / 8].toInt() and (1 shl (j % 8))) != 0) {
                        probes[j] = true
                    }
                }
            }
        }

        return store
    }
}
