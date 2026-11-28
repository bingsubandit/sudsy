import { describe, expect, test } from "vitest";
import { getPathSegments } from "../src";

describe("getPathSegments", () => {
  test("decodes each segment", () => {
    expect(getPathSegments("/a%20b/c/")).toEqual(["a b", "c"]);
    expect(getPathSegments("https://x.dev/foo/%E2%9C%93?q=1#h")).toEqual([
      "foo",
      "✓",
    ]);
    expect(getPathSegments("")).toEqual([]);
  });
  test("keeps a malformed escape", () => {
    expect(getPathSegments("/bad%E0%A4%A/x")).toEqual(["bad%E0%A4%A", "x"]);
  });
});
