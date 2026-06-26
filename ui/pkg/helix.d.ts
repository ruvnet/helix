/* tslint:disable */
/* eslint-disable */

/**
 * Run the grounded-answer pipeline. Input: an [`AnalyzePayload`] as JSON.
 * Output: a `helix_pipeline::AnswerOutcome` as JSON (`abstained` or `answered`).
 */
export function analyze_json(payload: string): string;

/**
 * Biological-age estimate (ADR-034): input PhenoInputs JSON → BioAge + disclaimer.
 */
export function bioage_json(inputs: string): string;

/**
 * Compose a decomposable 0–100 health score. Input: an array of `SubScore` as
 * JSON. Output: a `HealthScore` as JSON.
 */
export function compose_score_json(subscores: string): string;

/**
 * Focus areas (ADR-032): input `{records, now, config}` JSON → ranked focus items.
 */
export function focus_json(payload: string): string;

/**
 * Ingest a user-owned genome profile (ADR-021): returns `{records, advisories}` —
 * GENO-* records plus "verify with your prescriber" pharmacogenomic advisories.
 */
export function genome_profile_json(profile: string): string;

/**
 * The non-diagnostic disclaimer that must accompany any ruv-neural signal.
 */
export function neural_disclaimer(): string;

/**
 * Ingest a `ruv-neural` signed session (JSON) and return the provenance-tagged
 * records it maps to (JSON array), so EEG/40 Hz entrainment signals flow into
 * the same dossier as labs — as a research/screening signal (ADR-014 framing).
 */
export function neural_session_to_records_json(session: string): string;

/**
 * Gate an OCR'd lab document (ADR-022): returns the gated outcomes
 * (accepted records / queued candidates with reasons). `floor` is the minimum
 * OCR confidence to accept.
 */
export function ocr_ingest_json(document: string, floor: number): string;

/**
 * The red-flag threshold registry version currently in force (ADR-009).
 */
export function redflag_registry_version(): string;

/**
 * Ingest a RuView WiFi-CSI reading (ADR-020): returns `{records, flags}` —
 * vital ProvRecords plus Escalation Guardian screening flags.
 */
export function sensing_reading_json(reading: string): string;

/**
 * Score timeline (ADR-031): input `{snapshots, flat_band}` JSON → versioned
 * ScorePoints + trend + change-point.
 */
export function timeline_json(payload: string): string;

/**
 * Crate version string for the UI footer / diagnostics.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly analyze_json: (a: number, b: number) => [number, number, number, number];
    readonly bioage_json: (a: number, b: number) => [number, number, number, number];
    readonly compose_score_json: (a: number, b: number) => [number, number, number, number];
    readonly focus_json: (a: number, b: number) => [number, number, number, number];
    readonly genome_profile_json: (a: number, b: number) => [number, number, number, number];
    readonly neural_disclaimer: () => [number, number];
    readonly neural_session_to_records_json: (a: number, b: number) => [number, number, number, number];
    readonly ocr_ingest_json: (a: number, b: number, c: number) => [number, number, number, number];
    readonly redflag_registry_version: () => [number, number];
    readonly sensing_reading_json: (a: number, b: number) => [number, number, number, number];
    readonly timeline_json: (a: number, b: number) => [number, number, number, number];
    readonly version: () => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
