package com.jkjamies.omnivore.agent.runtime

import java.io.DataInputStream
import java.io.File
import java.io.FileInputStream

/**
 * Reads probe map from .probes binary files.
 * Used by the reporter to map probe indices back to source lines.
 */
object ProbeMapReader {

    private const val MAGIC = "OMNIPROB"

    fun read(file: File): ProbeMap {
        val probeMap = ProbeMap()

        DataInputStream(FileInputStream(file).buffered()).use { input ->
            val magic = ByteArray(8)
            input.readFully(magic)
            check(String(magic) == MAGIC) { "Invalid probe map file: bad magic" }

            val version = input.readShort()
            check(version.toInt() == 1) { "Unsupported version: $version" }

            val classCount = input.readInt()

            for (i in 0 until classCount) {
                val classId = input.readLong()
                val className = input.readUTF()
                val sourceFile = input.readUTF().ifEmpty { null }

                val classMap = probeMap.getOrCreateClassMap(classId, className, sourceFile)

                val probeCount = input.readInt()
                for (j in 0 until probeCount) {
                    val probeIndex = input.readInt()
                    val lineNumber = input.readInt()
                    val methodName = input.readUTF()
                    val methodDesc = input.readUTF()
                    val type = ProbeType.entries[input.readByte().toInt()]

                    classMap.addProbe(probeIndex, lineNumber, methodName, methodDesc, type)
                }
            }
        }

        return probeMap
    }
}
