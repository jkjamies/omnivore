package com.example.app.util

import io.kotest.assertions.throwables.shouldThrow
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe

class CalculatorTest : FunSpec({

    val calc = Calculator()

    test("add works") {
        calc.add(2, 3) shouldBe 5
    }

    test("subtract works") {
        calc.subtract(3, 2) shouldBe 1
    }

    // multiply is intentionally not tested

    test("divide works") {
        calc.divide(10, 2) shouldBe 5
    }

    test("divide by zero throws") {
        shouldThrow<IllegalArgumentException> {
            calc.divide(1, 0)
        }
    }

    test("classify negative") {
        calc.classify(-5) shouldBe "negative"
    }

    test("classify zero") {
        calc.classify(0) shouldBe "zero"
    }

    test("classify small") {
        calc.classify(5) shouldBe "small"
    }

    // medium and large branches intentionally not tested

    test("fibonacci base cases") {
        calc.fibonacci(0) shouldBe 0
        calc.fibonacci(1) shouldBe 1
    }

    test("fibonacci computes correctly") {
        calc.fibonacci(6) shouldBe 8
    }
})
