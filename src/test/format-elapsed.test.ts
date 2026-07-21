import { describe, expect, it } from "vitest"
import { formatElapsed } from "@/lib/utils"

describe("formatElapsed", () => {
  it("formats zero", () => {
    const result = formatElapsed(0)
    expect(result.hours).toBe("00")
    expect(result.minutes).toBe("00")
    expect(result.seconds).toBe("00")
  })

  it("formats 3661 seconds (1h 1m 1s)", () => {
    const result = formatElapsed(3661)
    expect(result.hours).toBe("01")
    expect(result.minutes).toBe("01")
    expect(result.seconds).toBe("01")
  })

  it("formats 86399 seconds (23h 59m 59s)", () => {
    const result = formatElapsed(86399)
    expect(result.hours).toBe("23")
    expect(result.minutes).toBe("59")
    expect(result.seconds).toBe("59")
  })
})
