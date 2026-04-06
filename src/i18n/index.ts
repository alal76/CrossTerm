import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en.json";
import ar from "./ar.json";
import he from "./he.json";

export const RTL_LOCALES = ["ar", "he", "fa", "ur"];

export function isRtlLocale(lang: string): boolean {
  return RTL_LOCALES.includes(lang);
}

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    ar: { translation: ar },
    he: { translation: he },
  },
  lng: "en",
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
