package com.jkjamies.omnivore.gradle.configuration

import com.jkjamies.omnivore.gradle.OmnivoreExtension
import org.gradle.api.Project

/**
 * Registers an ASM bytecode transformation via AGP's Instrumentation API.
 *
 * This ensures application classes are instrumented with Omnivore probes at build time,
 * since Android (ART) does not support -javaagent runtime instrumentation.
 *
 * Uses AGP's AndroidComponentsExtension to register a ClassVisitorFactory that
 * applies the same ASM transformation as OmnivoreClassTransformer.
 */
object OmnivoreTransformConfigurator {

    /**
     * Register the build-time bytecode transform for Android projects.
     *
     * This hooks into AGP's Instrumentation API which processes .class files
     * before they are dexed and packaged into the APK.
     *
     * We use reflection to interact with AGP APIs since they are compileOnly
     * dependencies — this keeps the plugin usable in non-Android projects.
     */
    fun configure(project: Project, extension: OmnivoreExtension) {
        try {
            // Access AndroidComponentsExtension via reflection
            val componentsExt = project.extensions.findByName("androidComponents") ?: return

            val onVariantsMethod = componentsExt.javaClass.getMethod(
                "onVariants",
                Class.forName("com.android.build.api.variant.VariantSelector"),
                Class.forName("kotlin.jvm.functions.Function1")
            )

            // Get the selector() method to create a VariantSelector
            val selectorMethod = componentsExt.javaClass.getMethod("selector")
            val selector = selectorMethod.invoke(componentsExt)

            // Select all variants
            val allMethod = selector.javaClass.getMethod("all")
            val allSelector = allMethod.invoke(selector)

            // Register on each variant
            onVariantsMethod.invoke(componentsExt, allSelector, object : kotlin.jvm.functions.Function1<Any, Unit> {
                override fun invoke(variant: Any) {
                    configureVariant(project, variant, extension)
                }
            })
        } catch (e: Exception) {
            project.logger.info("Omnivore: Could not register AGP bytecode transform: ${e.message}")
            project.logger.info("Omnivore: Instrumented test coverage will require pre-instrumented classes.")
        }
    }

    private fun configureVariant(project: Project, variant: Any, extension: OmnivoreExtension) {
        try {
            // Get the variant's instrumentation object
            val instrumentationMethod = variant.javaClass.getMethod("getInstrumentation")
            val instrumentation = instrumentationMethod.invoke(variant)

            // Register our AsmClassVisitorFactory
            // instrumentation.transformClassesWith(OmnivoreClassVisitorFactory::class.java, InstrumentationScope.ALL) { params -> ... }
            val transformMethod = instrumentation.javaClass.methods.find {
                it.name == "transformClassesWith"
            }

            if (transformMethod != null) {
                val factoryClass = Class.forName(
                    "com.jkjamies.omnivore.gradle.transform.OmnivoreClassVisitorFactory"
                )
                val scopeClass = Class.forName(
                    "com.android.build.api.instrumentation.InstrumentationScope"
                )
                val allScope = scopeClass.enumConstants.find { it.toString() == "ALL" }

                transformMethod.invoke(
                    instrumentation,
                    factoryClass,
                    allScope,
                    object : kotlin.jvm.functions.Function1<Any, Unit> {
                        override fun invoke(params: Any) {
                            // Configure transform parameters
                            configureTransformParams(params, extension)
                        }
                    }
                )

                project.logger.lifecycle("Omnivore: Registered build-time bytecode transform for variant")
            }
        } catch (e: Exception) {
            project.logger.info("Omnivore: Could not configure variant transform: ${e.message}")
        }
    }

    private fun configureTransformParams(params: Any, extension: OmnivoreExtension) {
        try {
            // Set includes
            val includesMethod = params.javaClass.getMethod("getIncludes")
            val includesProp = includesMethod.invoke(params)
            includesProp.javaClass.getMethod("set", Any::class.java)
                .invoke(includesProp, extension.includes.get())

            // Set excludes
            val excludesMethod = params.javaClass.getMethod("getExcludes")
            val excludesProp = excludesMethod.invoke(params)
            excludesProp.javaClass.getMethod("set", Any::class.java)
                .invoke(excludesProp, extension.excludes.get())

            // Set compose filter
            val composeMethod = params.javaClass.getMethod("getComposeFilterEnabled")
            val composeProp = composeMethod.invoke(params)
            composeProp.javaClass.getMethod("set", Any::class.java)
                .invoke(composeProp, extension.composeFilter.enabled.getOrElse(true))
        } catch (e: Exception) {
            // Parameters are optional — defaults will be used
        }
    }
}
