import { describe, expect, test } from "vitest";
import { getOrigin } from "../src";

describe("getOrigin", () => {
  test("absolute URLs", () => {
    expect(getOrigin("https://user@example.com:8080/path?x=1")).toBe(
      "https://example.com:8080",
    );
    expect(getOrigin("http://localhost")).toBe("http://localhost");
  });
  test("relative input", () => {
    expect(getOrigin("/relative")).toBe("");
  });
});
