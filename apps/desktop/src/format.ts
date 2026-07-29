import type { Locale } from "./i18n/messages";

const BYTE_BASE = 1_024n;
const BYTE_UNITS = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] as const;

export function formatInteger(value: string | bigint, locale: Locale): string {
  return BigInt(value).toLocaleString(locale);
}
export function formatBytes(value: string, locale: Locale): string {
  const bytes = BigInt(value);
  let unitIndex = 0;
  let divisor = 1n;

  while (
    unitIndex < BYTE_UNITS.length - 1 &&
    bytes >= divisor * BYTE_BASE
  ) {
    divisor *= BYTE_BASE;
    unitIndex += 1;
  }

  if (unitIndex === 0) {
    return `${formatInteger(bytes, locale)} ${BYTE_UNITS[unitIndex]}`;
  }

  const whole = bytes / divisor;
  const decimal = ((bytes % divisor) * 10n) / divisor;
  const separator = locale === "pt-BR" ? "," : ".";
  const fraction = decimal === 0n ? "" : `${separator}${decimal.toString()}`;
  return `${formatInteger(whole, locale)}${fraction} ${BYTE_UNITS[unitIndex]}`;
}
