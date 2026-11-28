import { describe, expect, it } from "vitest";
import { join, resolve } from "../src";
import { relativeToCwd } from "../src/utils";

describe("relativeToCwd", () => {
  it("is relative to the working directory", () => {
    expect(relativeToCwd(join(resolve(), "a/b"))).toBe("a/b");
    expect(relativeToCwd(resolve())).toBe("");
  });
});
