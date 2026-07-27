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
    private const val SUPPORTED_VERSION = 3

    fun read(file: File): ProbeMap {
        val probeMap = ProbeMap()

        DataInputStream(FileInputStream(file).buffered()).use { input ->
            val magic = ByteArray(8)
            input.readFully(magic)
            check(String(magic) == MAGIC) { "Invalid probe map file: bad magic" }

            val version = input.readShort().toInt()
            // v1/v2 are rejected, not upgraded. In v3 branch probes moved onto
            // control-flow edges, so probe indices mean something different;
            // reading an old file against new execution data would silently
            // attribute coverage to the wrong lines. Failing loudly sends the
            // user to a clean build, which is the correct fix.
            check(version == SUPPORTED_VERSION) {
                if (version in 1 until SUPPORTED_VERSION) {
                    "Probe map ${file.name} was written by an older Omnivore " +
                        "(format v$version, expected v$SUPPORTED_VERSION). Branch probe layout " +
                        "changed; run a clean build to regenerate coverage data."
                } else {
                    "Unsupported probe map version: $version"
                }
            }

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
                    val isComposable = input.readByte().toInt() == 1
                    val branchGroup = input.readInt()

                    classMap.addProbe(
                        probeIndex, lineNumber, methodName, methodDesc, type, isComposable, branchGroup
                    )
                }
            }
        }

        return probeMap
    }
}
