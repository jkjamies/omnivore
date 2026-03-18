package com.jkjamies.omnivore.agent.runtime

import java.io.DataOutputStream
import java.io.File
import java.io.FileOutputStream

/**
 * Writes execution data to the .omnivore binary format.
 *
 * File format:
 *   Header: "OMNIVORE" (8 bytes) + version (2 bytes)
 *   For each class:
 *     - classId: Long (8 bytes)
 *     - className length: Int (4 bytes)
 *     - className: UTF-8 bytes
 *     - probeCount: Int (4 bytes)
 *     - probes: packed bits (ceil(probeCount/8) bytes)
 */
object ExecutionDataWriter {

    private const val MAGIC = "OMNIVORE"
    private const val VERSION: Short = 1

    fun write(file: File, dataStore: ExecutionDataStore) {
        file.parentFile?.mkdirs()

        DataOutputStream(FileOutputStream(file).buffered()).use { out ->
            // Header
            out.writeBytes(MAGIC)
            out.writeShort(VERSION.toInt())

            // Class data
            val allData = dataStore.getAllData()
            out.writeInt(allData.size)

            for (data in allData) {
                out.writeLong(data.classId)
                out.writeUTF(data.className)
                out.writeInt(data.probes.size)

                // Pack probes as bits
                val packedSize = (data.probes.size + 7) / 8
                val packed = ByteArray(packedSize)
                for (i in data.probes.indices) {
                    if (data.probes[i]) {
                        packed[i / 8] = (packed[i / 8].toInt() or (1 shl (i % 8))).toByte()
                    }
                }
                out.write(packed)
            }
        }
    }
}
