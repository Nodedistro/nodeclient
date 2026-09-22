import { describe, it, expect } from "vitest";
import { defaultMemory, instanceSchema } from "./models";
describe("instance configuration", () => {
  const sample = {
    id: "main",
    name: "Main",
    minecraftVersion: "1.21",
    loader: { type: "vanilla" },
    java: { mode: "automatic", path: null },
    memory: { minimumMb: 1024, maximumMb: 4096 },
  };
  it("validates real configurations", () =>
    expect(instanceSchema.safeParse(sample).success).toBe(true));
  it("rejects traversal and inverted RAM", () => {
    expect(
      instanceSchema.safeParse({ ...sample, id: "../outside" }).success,
    ).toBe(false);
    expect(
      instanceSchema.safeParse({
        ...sample,
        memory: { minimumMb: 4096, maximumMb: 1024 },
      }).success,
    ).toBe(false);
  });
  it("accepts fabric loader versions", () => {
    expect(
      instanceSchema.safeParse({
        ...sample,
        loader: { type: "fabric", version: "0.19.5" },
      }).success,
    ).toBe(true);
  });
  it("accepts forge/quilt/neoforge with versions", () => {
    for (const type of ["forge", "quilt", "neoforge"] as const) {
      expect(
        instanceSchema.safeParse({
          ...sample,
          loader: { type, version: "1.0.0" },
        }).success,
      ).toBe(true);
    }
  });
  it("rejects unknown loaders", () => {
    expect(
      instanceSchema.safeParse({
        ...sample,
        loader: { type: "optifine" },
      }).success,
    ).toBe(false);
  });
  it("rejects forge without a version", () => {
    expect(
      instanceSchema.safeParse({
        ...sample,
        loader: { type: "forge" },
      }).success,
    ).toBe(false);
  });
  it("leaves room for the operating system", () => {
    expect(defaultMemory(8192)).toBe(2048);
    expect(defaultMemory(32768)).toBe(4096);
  });
});
