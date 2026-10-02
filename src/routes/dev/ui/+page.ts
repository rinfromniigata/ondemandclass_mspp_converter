// 基本部品の確認用ページ。開発時（bun run dev / bun tauri dev）のみ開ける
import { dev } from "$app/environment";
import { error } from "@sveltejs/kit";

export function load() {
  if (!dev) error(404, "Not Found");
}
