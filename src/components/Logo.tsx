import { t } from "../i18n";
import type { LocaleId } from "../types";

interface LogoProps {
  locale: LocaleId;
}

export function Logo({ locale }: LogoProps) {
  return (
    <div className="brand" aria-label="Token Anxiety">
      <div className="brand-mark">
        <span />
        <span />
        <span />
      </div>
      <div>
        <strong>Token Anxiety</strong>
        <small>{t(locale, "brandSubtitle")}</small>
      </div>
    </div>
  );
}
