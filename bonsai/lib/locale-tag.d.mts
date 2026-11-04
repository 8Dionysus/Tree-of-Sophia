export type BonsaiLocale = "en" | "ru" | "es" | "de" | "zh-Hans";
export declare const BONSAI_LOCALES: readonly BonsaiLocale[];
export declare const DEFAULT_LOCALE: "en";
export declare function resolveLocale(value: unknown): BonsaiLocale;
export declare function resolveLocale<F>(value: unknown, fallback: F): BonsaiLocale | F;
