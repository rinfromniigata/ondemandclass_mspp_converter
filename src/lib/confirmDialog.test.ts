import { get } from "svelte/store";
import { afterEach, describe, expect, it } from "vitest";
import { pendingConfirm, requestConfirm, type ConfirmRequest } from "./confirmDialog";

const req = (title: string): ConfirmRequest => ({
  title,
  message: "出力先に同じ名前のファイルがあります。",
  details: ["lec_audio.m4a"],
  confirmLabel: "上書きする",
  cancelLabel: "キャンセル",
});

describe("confirmDialog", () => {
  afterEach(() => {
    // モジュール内の状態を次のテストへ持ち越さない
    get(pendingConfirm)?.resolve(false);
  });

  it("要求するとストアに要求内容が入る", () => {
    void requestConfirm(req("上書きしますか？"));
    expect(get(pendingConfirm)).toMatchObject(req("上書きしますか？"));
  });

  it.each([true, false])("resolve(%s) で Promise が解決し、ストアが null に戻る", async (answer) => {
    const p = requestConfirm(req("A"));
    get(pendingConfirm)?.resolve(answer);
    await expect(p).resolves.toBe(answer);
    expect(get(pendingConfirm)).toBeNull();
  });

  it("表示中に新しい要求が来たら、前の要求を false で解決して置き換える", async () => {
    const first = requestConfirm(req("A"));
    const second = requestConfirm(req("B"));

    await expect(first).resolves.toBe(false);
    expect(get(pendingConfirm)?.title).toBe("B");

    get(pendingConfirm)?.resolve(true);
    await expect(second).resolves.toBe(true);
    expect(get(pendingConfirm)).toBeNull();
  });

  it("置き換えられた古い要求の resolve を後から呼んでも、新しい要求に影響しない", async () => {
    void requestConfirm(req("A"));
    const stale = get(pendingConfirm);
    const second = requestConfirm(req("B"));

    stale?.resolve(true);
    expect(get(pendingConfirm)?.title).toBe("B");

    get(pendingConfirm)?.resolve(false);
    await expect(second).resolves.toBe(false);
  });

  it("2回目以降の resolve は無視する", async () => {
    const p = requestConfirm(req("A"));
    const entry = get(pendingConfirm);
    entry?.resolve(true);
    entry?.resolve(false);
    await expect(p).resolves.toBe(true);
  });
});
