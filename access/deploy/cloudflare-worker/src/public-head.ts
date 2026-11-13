import { resolveLocale } from "../../../../bonsai/lib/locale-tag.mjs";

// A stand's page names itself in English, the site's default. A link that asks for Russian gets
// its title, description and card in Russian; a link in another language of the site keeps the
// English head under its own language tag. A shared link reads in the reader's language.
type Head = { title: string; og: string; description: string };
export const HEADS: Record<string, Record<"en" | "ru", Head>> = {
  home: {
    en: {
      title: "Tree of Sophia",
      og: "Tree of Sophia",
      description: "Thought grown from the sources. Each bonsai cultivates its own direction.",
    },
    ru: {
      title: "Древо Софии",
      og: "Древо Софии",
      description: "Мысль, выращенная из источников. Каждый бонсай культивирует своё направление.",
    },
  },
  zarathustra: {
    en: {
      title: "Zarathustra · Tree of Sophia",
      og: "Thus Spoke Zarathustra · Tree of Sophia",
      description: "Thus Spoke Zarathustra by Friedrich Nietzsche in the Tree of Sophia.",
    },
    ru: {
      title: "Заратустра · Древо Софии",
      og: "Так говорил Заратустра · Древо Софии",
      description: "«Так говорил Заратустра» Фридриха Ницше в Древе Софии.",
    },
  },
  antiquity: {
    en: {
      title: "Antiquity · Tree of Sophia",
      og: "Antiquity · Tree of Sophia",
      description: "The philosophy of ancient Greece and Rome in the Tree of Sophia.",
    },
    ru: {
      title: "Античность · Древо Софии",
      og: "Античность · Древо Софии",
      description: "Философия Древней Греции и Рима в Древе Софии.",
    },
  },
};
const SITE = { en: "Tree of Sophia", ru: "Древо Софии" } as const;
const OG_LOCALE = { en: "en_US", ru: "ru_RU" } as const;
const escapeHtml = (value: string) => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

/**
 * The stand page's head in the language its link asks for: Russian copy for Russian, English otherwise.
 * The tag is read as the browser reads it (bonsai/lib/locale-tag.mjs), so zh-CN or ru-RU name the same
 * language here as in the page; a tag the site does not have leaves the English page as it is.
 */
export function localizeHead(html: string, stand: string, requested: string | null): string {
  const heads = HEADS[stand], lang = resolveLocale(requested, null);
  if (!heads || !lang) return html;
  const copy = lang === "ru" ? "ru" : "en", head = heads[copy];
  const content = (pattern: RegExp, value: string) => (text: string) => text.replace(pattern, `$1${escapeHtml(value)}$2`);
  return [
    (text: string) => text.replace(/<html lang="[^"]*"/, `<html lang="${lang}"`),
    (text: string) => text.replace(/<title>[^<]*<\/title>/, `<title>${escapeHtml(head.title)}</title>`),
    content(/(<meta name="description" content=")[^"]*(")/, head.description),
    content(/(<meta property="og:description" content=")[^"]*(")/, head.description),
    content(/(<meta property="og:title" content=")[^"]*(")/, head.og),
    content(/(<meta property="og:site_name" content=")[^"]*(")/, SITE[copy]),
    content(/(<meta property="og:locale" content=")[^"]*(")/, OG_LOCALE[copy]),
    (text: string) => text.replace(/(<meta property="og:image" content="[^"]*\/og\/[a-z]+)-(?:ru|en)(\.jpg")/, `$1-${copy}$2`),
  ].reduce((text, step) => step(text), html);
}

