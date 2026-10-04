// Promise を返す確認ダイアログ用ストア。ConfirmDialog.svelte がストアを購読して表示する
import { writable, type Readable } from "svelte/store";

export interface ConfirmRequest {
  title: string;
  message: string;
  details?: string[];
  confirmLabel: string;
  cancelLabel: string;
}

export type PendingConfirm = ConfirmRequest & { resolve: (ok: boolean) => void };

const pending = writable<PendingConfirm | null>(null);
let current: PendingConfirm | null = null;

/** 表示中の確認要求。同時に1件だけ保持する */
export const pendingConfirm: Readable<PendingConfirm | null> = { subscribe: pending.subscribe };

/**
 * 確認要求を出し、利用者の選択（承諾なら true）で解決する Promise を返す。
 * 表示中に新しい要求が来た場合は、前の要求を false で解決してから置き換える
 */
export function requestConfirm(req: ConfirmRequest): Promise<boolean> {
  current?.resolve(false);

  return new Promise<boolean>((resolvePromise) => {
    const entry: PendingConfirm = {
      ...req,
      resolve: (ok) => {
        // 解決は1回だけ。表示中の要求が自分ならストアを null に戻す
        if (current !== entry) return;
        current = null;
        pending.set(null);
        resolvePromise(ok);
      },
    };
    current = entry;
    pending.set(entry);
  });
}
