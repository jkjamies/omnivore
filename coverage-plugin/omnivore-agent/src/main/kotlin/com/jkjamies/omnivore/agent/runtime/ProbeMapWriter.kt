package com.jkjamies.omnivore.agent.runtime

import java.io.DataOutputStream
import java.io.File
import java.io.FileOutputStream

/**
 * Writes the probe map to a binary .probes file.
 *
 * File format:
 *   Header: "OMNIPROB" (8 bytes) + version (2 bytes)
 *   classCount: Int
 *   For each class:
 *     classId: Long
 *     className: UTF
 *     sourceFile: UTF (empty string if null)
 *     probeCount: Int
 *     For each probe:
 *       probeIndex: Int
 *       lineNumber: Int
 *       methodName: UTF
 *       methodDesc: UTF
 *       type: Byte (0=LINE, 1=BRANCH)
 *       isComposable: Byte (0=false, 1=true)  [v2+]
 *       branchGroup: Int                      [v3+]
 *
 * Version history:
 *   v1 — initial
 *   v2 — added isComposable
 *   v3 — added branchGroup, and branch probes moved from the branch
 *        *instruction* to its outgoing *edges*. A v3 file therefore contains
 *        roughly twice as many branch probes as a v2 file for the same class,
 *        and the probe indices are entirely different. v1/v2 files are rejected
 *        rather than misread: silently pairing an old .probes with a new
 *        .omnivore would attribute coverage to the wrong lines.
 */
object ProbeMapWriter {

    private const val MAGIC = "OMNIPROB"
    private const val VERSION: Short = 3

    fun write(file: File, probeMap: ProbeMap) {
        file.parentFile?.mkdirs()

        DataOutputStream(FileOutputStream(file).buffered()).use { out ->
            out.writeBytes(MAGIC)
            out.writeShort(VERSION.toInt())

            val allMaps = probeMap.getAllClassMaps()
            out.writeInt(allMaps.size)

            for (classMap in allMaps) {
                out.writeLong(classMap.classId)
                out.writeUTF(classMap.className)
                out.writeUTF(classMap.sourceFile ?: "")

                val probes = classMap.getProbes()
                out.writeInt(probes.size)

                for (probe in probes) {
                    out.writeInt(probe.probeIndex)
                    out.writeInt(probe.lineNumber)
                    out.writeUTF(probe.methodName)
                    out.writeUTF(probe.methodDesc)
                    out.writeByte(probe.type.ordinal)
                    out.writeByte(if (probe.isComposable) 1 else 0)
                    out.writeInt(probe.branchGroup)
                }
            }
        }
    }
}
