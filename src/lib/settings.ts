// 設定の読み込み・検証結果ストア（スペック5.2）
import { writable } from "svelte/store";
import { commands, type Commands, type SettingsStatus } from "./tauriCommands";

export const settingsStatus = writable<SettingsStatus | null>(null);

/** 設定を読み込み直してストアに入れる。invoke 自体の失敗も ok: false として格納する */
export async function reloadSettings(cmd: Pick<Commands, "loadAndValidateSettings"> = commands): Promise<void> {
  try {
    settingsStatus.set(await cmd.loadAndValidateSettings());
  } catch (e) {
    settingsStatus.set({ ok: false, settingsPath: "", settings: null, errors: [String(e)] });
  }
}
