import { describe, it, expect } from "vitest";
import { SPICE_SCANCODES, keyboardEventToScancode } from "@/utils/spiceScancodes";

function fakeKeyEvent(code: string): KeyboardEvent {
  return { code } as KeyboardEvent;
}

describe("spiceScancodes", () => {
  it("maps letter keys to their PC/AT set-1 base scancode", () => {
    expect(SPICE_SCANCODES.KeyA).toBe(0x1e);
    expect(SPICE_SCANCODES.KeyZ).toBe(0x2c);
  });

  it("maps digit keys to their PC/AT set-1 base scancode", () => {
    expect(SPICE_SCANCODES.Digit1).toBe(0x2);
    expect(SPICE_SCANCODES.Digit0).toBe(0xb);
  });

  it("marks extended keys with the 0x100 flag rather than a raw 0xe0 prefix", () => {
    // ArrowUp is 0xe0 0x48 on the wire; SPICE's convention drops the 0xe0
    // prefix and ORs in 0x100 (per spice-gtk's documented API contract).
    expect(SPICE_SCANCODES.ArrowUp).toBe(0x148);
    expect(SPICE_SCANCODES.ControlRight).toBe(0x11d);
    expect(SPICE_SCANCODES.Delete).toBe(0x153);
  });

  it("does not flag non-extended keys", () => {
    expect(SPICE_SCANCODES.ControlLeft).toBeLessThan(0x100);
    expect(SPICE_SCANCODES.Enter).toBeLessThan(0x100);
  });

  it("keyboardEventToScancode resolves a known code and returns null for an unknown one", () => {
    expect(keyboardEventToScancode(fakeKeyEvent("KeyA"))).toBe(0x1e);
    expect(keyboardEventToScancode(fakeKeyEvent("AudioVolumeUp"))).toBeNull();
  });
});
