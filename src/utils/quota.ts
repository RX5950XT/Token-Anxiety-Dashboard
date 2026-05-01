import type { AccountStatus, DashboardState, QuotaWindow, UsageAccount } from "../types";
import { t } from "../i18n";
import type { LocaleId } from "../types";

export interface WindowProgress {
  percentage: number;
  resetLabel: string;
  status: AccountStatus;
}

export function clampPercentage(value: number): number {
  if (Number.isNaN(value) || !Number.isFinite(value)) {
    return 0;
  }

  return Math.min(100, Math.max(0, Math.round(value)));
}

export function getWindowProgress(window: QuotaWindow, now = Date.now(), locale: LocaleId = "zh-TW"): WindowProgress {
  const percentage = clampPercentage((window.used / window.limit) * 100);
  return {
    percentage,
    resetLabel: formatDurationUntil(window.resetAt, now, locale),
    status: statusFromPercentage(percentage),
  };
}

export function statusFromPercentage(percentage: number): AccountStatus {
  if (percentage >= 100) {
    return "limited";
  }

  if (percentage >= 80) {
    return "warning";
  }

  return "available";
}

export function deriveAccountStatus(account: UsageAccount, now = Date.now()): AccountStatus {
  if (account.status === "disconnected") {
    return "disconnected";
  }

  const statuses = account.windows.map((window) => getWindowProgress(window, now).status);
  if (statuses.includes("limited")) {
    return "limited";
  }

  if (statuses.includes("warning")) {
    return "warning";
  }

  return "available";
}

export function formatDurationUntil(isoDate: string, now = Date.now(), locale: LocaleId = "zh-TW"): string {
  const target = new Date(isoDate).getTime();
  if (Number.isNaN(target)) {
    return t(locale, "needsCalibration");
  }

  const diff = target - now;
  if (diff <= 0) {
    return t(locale, "readyToRefresh");
  }

  const totalMinutes = Math.ceil(diff / 60_000);
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;

  if (days > 0) {
    return locale === "zh-TW" ? `${days} ${t(locale, "day")} ${hours} ${t(locale, "hour")}` : `${days}${t(locale, "day")} ${hours}${t(locale, "hour")}`;
  }

  if (hours > 0) {
    return locale === "zh-TW" ? `${hours} ${t(locale, "hour")} ${minutes} ${t(locale, "minute")}` : `${hours}${t(locale, "hour")} ${minutes}${t(locale, "minute")}`;
  }

  return locale === "zh-TW" ? `${minutes} ${t(locale, "minute")}` : `${minutes}${t(locale, "minute")}`;
}

export function summarizeDashboard(state: DashboardState, now = Date.now(), locale: LocaleId = state.settings.locale) {
  const statuses = state.accounts.map((account) => deriveAccountStatus(account, now));
  const limitedCount = statuses.filter((status) => status === "limited").length;
  const warningCount = statuses.filter((status) => status === "warning").length;
  const nextReset = state.accounts
    .flatMap((account) => account.windows)
    .map((window) => new Date(window.resetAt).getTime())
    .filter((time) => !Number.isNaN(time) && time > now)
    .sort((a, b) => a - b)[0];

  return {
    accountCount: state.accounts.length,
    availableCount: statuses.filter((status) => status === "available").length,
    attentionCount: limitedCount + warningCount,
    nextResetLabel: nextReset ? formatDurationUntil(new Date(nextReset).toISOString(), now, locale) : t(locale, "noData"),
  };
}

export function formatWindowLabel(window: QuotaWindow, locale: LocaleId): string {
  if (window.kind === "daily") {
    return t(locale, "dailyUsage");
  }

  if (window.kind === "monthly") {
    return t(locale, "monthlyWindow");
  }

  if (window.kind === "weekly") {
    return t(locale, "weeklyWindow");
  }

  return t(locale, "fiveHourWindow");
}
