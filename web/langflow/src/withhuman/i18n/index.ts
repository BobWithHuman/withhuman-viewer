// Korean from the first launch on, through Langflow's own i18n (react-i18next, src/i18n.ts):
// Langflow's `ko.json` with WithHuman's overrides on top (./ko.ts: short nouns in place of
// sentences, Langflow's name taken out), registered before Langflow's entry asks for it, and
// `languagePreference` kept at "ko" so a reload stays Korean.

import i18n from "@/i18n";
import ko from "@/locales/ko.json";
import { KO } from "./ko";

try {
  localStorage.setItem("languagePreference", "ko");
} catch {}

// An override of "" drops a description line: Langflow sets `returnEmptyString: false`, which
// would fall back to English and then to the key itself.
i18n.options.returnEmptyString = true;
i18n.addResourceBundle("ko", "translation", { ...ko, ...KO }, true, true);
// English stays the fallback for keys Langflow has not translated; our overrides cover the ones
// shown on our screens, so they are put in English's place too.
i18n.addResourceBundle("en", "translation", KO, true, true);
void i18n.changeLanguage("ko");
