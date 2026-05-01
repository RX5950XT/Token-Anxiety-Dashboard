import { defaultAnimateLayoutChanges, useSortable } from "@dnd-kit/sortable";
import type { CSSProperties } from "react";
import { providerMeta } from "../data/providers";
import { t } from "../i18n";
import type { LocaleId, ProviderEnvironment, UsageAccount } from "../types";
import { deriveAccountStatus, formatWindowLabel, getWindowProgress } from "../utils/quota";

interface AccountCardProps {
  account: UsageAccount;
  environment?: ProviderEnvironment;
  locale: LocaleId;
  overlay?: boolean;
}

const statusKey = {
  available: "statusAvailable",
  warning: "statusWarning",
  limited: "statusLimited",
  disconnected: "statusDisconnected",
} as const;

const accuracyKey = {
  official: "accuracyOfficial",
  local: "accuracyLocal",
  estimated: "accuracyEstimated",
  manual: "accuracyManual",
} as const;

function AccountCardBody({
  account,
  environment,
  locale,
  className,
  attributes,
  listeners,
}: AccountCardProps & {
  className: string;
  attributes?: object;
  listeners?: object;
}) {
  const meta = providerMeta[account.provider];
  const status = deriveAccountStatus(account);
  const style = {
    "--accent": meta.accent,
  } as CSSProperties;

  return (
    <article
      className={`${className} ${status}`}
      style={style}
      aria-label={`${meta.label} ${t(locale, "dragToSort")}`}
      {...attributes}
      {...listeners}
    >
      <header>
        <span className="provider-pill">{meta.label}</span>
        <span className="status-pill">{t(locale, statusKey[status])}</span>
      </header>
      <div className="quota-list">
        {account.windows.length === 0 ? (
          <div className="quota-empty">{t(locale, "quotaUnavailable")}</div>
        ) : account.windows.map((window) => {
          const progress = getWindowProgress(window, Date.now(), locale);

          return (
            <div className="quota-row" key={window.id}>
              <div className="quota-head">
                <strong>{formatWindowLabel(window, locale)}</strong>
                <span>{progress.resetLabel}</span>
              </div>
              <div className="progress-track">
                <span style={{ width: `${progress.percentage}%` }} />
              </div>
              <div className="quota-foot">
                <span>{progress.percentage}% {t(locale, "used")}</span>
              </div>
            </div>
          );
        })}
      </div>
      <footer className="card-footer">
        <span>{t(locale, "sourceStatus")}：{environment?.detected ? t(locale, "detected") : t(locale, "notDetected")}</span>
        <span>{t(locale, "confidence")}：{t(locale, accuracyKey[account.accuracy])}</span>
      </footer>
    </article>
  );
}

export function AccountCard({ account, environment, locale, overlay = false }: AccountCardProps) {
  if (overlay) {
    return <AccountCardBody account={account} environment={environment} locale={locale} className="account-card overlay-card" />;
  }

  const sortable = useSortable({
    id: account.id,
    transition: {
      duration: 110,
      easing: "cubic-bezier(0.22, 1, 0.36, 1)",
    },
    animateLayoutChanges(args) {
      return defaultAnimateLayoutChanges(args);
    },
  });

  return (
    <div
      ref={sortable.setNodeRef}
      style={{
        transform: sortable.transform
          ? `translate3d(${Math.round(sortable.transform.x)}px, ${Math.round(sortable.transform.y)}px, 0)`
          : undefined,
        transition: sortable.isDragging ? "none" : sortable.transition,
      }}
      className={`sortable-shell ${sortable.isDragging ? "is-sorting-ghost" : ""}`}
    >
      <AccountCardBody
        account={account}
        environment={environment}
        locale={locale}
        className={`account-card ${sortable.isDragging ? "is-dragging" : ""}`}
        attributes={sortable.attributes}
        listeners={sortable.listeners}
      />
    </div>
  );
}
