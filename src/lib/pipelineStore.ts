import { writable } from "svelte/store";
import type { PipelineState } from "./steps/types";

export const pipelineState = writable<PipelineState>({ view: "idle" });
