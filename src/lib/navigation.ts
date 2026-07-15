import { trackedInvoke } from "@/lib/tauri"

export async function openWebPanel(): Promise<void> {
  await trackedInvoke("open_web_panel")
}

export async function openExternalUrl(url: string): Promise<void> {
  await trackedInvoke("open_external_url", { url })
}
